import { useState } from "react";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { DocumentDetail, DocumentPage } from "../../types/note";
import * as notesService from "../../services/notes";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  IconButton,
} from "../ui";
import {
  AddNoteIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  CopyIcon,
  FolderIcon,
  PencilIcon,
  RefreshCwIcon,
  TrashIcon,
} from "../icons";
import { FolderNameDialog } from "../notes/FolderNameDialog";

const menuItemClass =
  "px-3 py-1.5 text-sm text-text cursor-pointer outline-none hover:bg-bg-muted focus:bg-bg-muted flex items-center gap-2 rounded-sm";

const menuSeparatorClass = "h-px bg-border my-1";

interface DocumentPagesSidebarProps {
  document: DocumentDetail;
  currentNoteId: string;
  mode: "page" | "all";
  onModeChange: (mode: "page" | "all") => Promise<void> | void;
  onSelectPage: (id: string) => Promise<void>;
  onDocumentChange: (document: DocumentDetail) => void;
  onRefreshNotes: () => Promise<void>;
  onNormalizeDocument: () => Promise<void>;
  isDocumentBusy?: boolean;
}

export function DocumentPagesSidebar({
  document,
  currentNoteId,
  mode,
  onModeChange,
  onSelectPage,
  onDocumentChange,
  onRefreshNotes,
  onNormalizeDocument,
  isDocumentBusy = false,
}: DocumentPagesSidebarProps) {
  const [renamingPage, setRenamingPage] = useState<DocumentPage | null>(null);
  const [deletingPage, setDeletingPage] = useState<DocumentPage | null>(null);
  const [isWorking, setIsWorking] = useState(false);
  const isBusy = isWorking || isDocumentBusy;

  async function handleCreatePage() {
    if (isBusy) return;
    setIsWorking(true);
    try {
      const next = await notesService.createDocumentPage(document.path);
      onDocumentChange(next);
      await onRefreshNotes();
      const created = next.pages[next.pages.length - 1];
      if (created) await onSelectPage(created.id);
    } catch (error) {
      console.error("Failed to create page:", error);
      toast.error("Failed to create page");
    } finally {
      setIsWorking(false);
    }
  }

  async function handleRenamePage(name: string) {
    if (!renamingPage || isBusy) return;
    setIsWorking(true);
    const oldIndex = renamingPage.index - 1;
    try {
      const next = await notesService.renameDocumentPage(
        document.path,
        renamingPage.file,
        name,
      );
      setRenamingPage(null);
      onDocumentChange(next);
      await onRefreshNotes();
      const renamed = next.pages[oldIndex];
      if (renamed && currentNoteId === renamingPage.id) {
        await onSelectPage(renamed.id);
      }
    } catch (error) {
      console.error("Failed to rename page:", error);
      toast.error("Failed to rename page");
    } finally {
      setIsWorking(false);
    }
  }

  async function handleDeletePage() {
    if (!deletingPage || isBusy) return;
    setIsWorking(true);
    try {
      const deletedIndex = deletingPage.index - 1;
      const next = await notesService.deleteDocumentPage(
        document.path,
        deletingPage.file,
      );
      setDeletingPage(null);
      onDocumentChange(next);
      await onRefreshNotes();
      if (currentNoteId === deletingPage.id) {
        const fallback = next.pages[Math.max(0, deletedIndex - 1)] ?? next.pages[0];
        if (fallback) await onSelectPage(fallback.id);
      }
    } catch (error) {
      console.error("Failed to delete page:", error);
      toast.error(error instanceof Error ? error.message : "Failed to delete page");
    } finally {
      setIsWorking(false);
    }
  }

  async function handleMovePage(page: DocumentPage, direction: "up" | "down") {
    if (isBusy) return;
    setIsWorking(true);
    const oldIndex = page.index - 1;
    try {
      const next = await notesService.moveDocumentPage(
        document.path,
        page.file,
        direction,
      );
      onDocumentChange(next);
      await onRefreshNotes();
      const newIndex =
        direction === "up"
          ? Math.max(0, oldIndex - 1)
          : Math.min(next.pages.length - 1, oldIndex + 1);
      const moved = next.pages[newIndex];
      if (moved && currentNoteId === page.id) {
        await onSelectPage(moved.id);
      }
    } catch (error) {
      console.error("Failed to move page:", error);
      toast.error("Failed to move page");
    } finally {
      setIsWorking(false);
    }
  }

  async function handleCopyPageFilepath(page: DocumentPage) {
    try {
      const folder = await notesService.getNotesFolder();
      if (!folder) return;
      await invoke("copy_to_clipboard", {
        text: `${folder}/${document.path}/${page.file}`,
      });
      toast.success("Page filepath copied");
    } catch (error) {
      console.error("Failed to copy page filepath:", error);
      toast.error("Failed to copy filepath");
    }
  }

  async function handleOpenDocumentFolder() {
    try {
      const folder = await notesService.getNotesFolder();
      if (!folder) return;
      await invoke("open_in_file_manager", {
        path: `${folder}/${document.path}`,
      });
    } catch (error) {
      console.error("Failed to open Document folder:", error);
      toast.error("Failed to open Document folder");
    }
  }

  return (
    <aside className="w-52 shrink-0 border-r border-border bg-bg-secondary/70 flex flex-col">
      <div className="px-3 py-2.5 border-b border-border">
        <div className="text-sm font-medium text-text truncate" title={document.title}>
          {document.title}
        </div>
        <div className="text-xs text-text-muted">
          {document.pages.length} {document.pages.length === 1 ? "page" : "pages"}
        </div>
        <div className="mt-2 grid grid-cols-2 gap-1 rounded-md bg-bg-muted p-0.5">
          <button
            type="button"
            className={`h-7 rounded text-xs font-medium transition-colors ${
              mode === "page"
                ? "bg-bg text-text shadow-sm"
                : "text-text-muted hover:text-text"
            }`}
            disabled={isBusy}
            onClick={() => onModeChange("page")}
          >
            Page
          </button>
          <button
            type="button"
            className={`h-7 rounded text-xs font-medium transition-colors ${
              mode === "all"
                ? "bg-bg text-text shadow-sm"
                : "text-text-muted hover:text-text"
            }`}
            disabled={isBusy}
            onClick={() => onModeChange("all")}
          >
            All Pages
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-1.5 space-y-1">
        {document.pages.map((page, index) => {
          const isSelected = page.id === currentNoteId;
          return (
            <ContextMenu.Root key={page.file}>
              <ContextMenu.Trigger asChild>
                <div
                  className={`group rounded-md border ${
                    isSelected
                      ? "border-border bg-bg-muted"
                      : "border-transparent hover:bg-bg-muted"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => onSelectPage(page.id)}
                    className="w-full px-2 py-1.5 text-left"
                  >
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] text-text-muted tabular-nums">
                        {String(page.index).padStart(2, "0")}
                      </span>
                      <span className="min-w-0 flex-1 truncate text-sm text-text">
                        {page.title}
                      </span>
                    </div>
                    <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-text-muted">
                      <span>{page.wordCount} words</span>
                      {page.overflow && (
                        <span className="text-amber-600 dark:text-amber-400">
                          overflow
                        </span>
                      )}
                    </div>
                  </button>
                  <div className="hidden group-hover:flex px-1.5 pb-1.5 gap-1">
                    <IconButton
                      onClick={() => handleMovePage(page, "up")}
                      disabled={index === 0 || isBusy}
                      title="Move page up"
                      className="h-6 w-6"
                    >
                      <ArrowUpIcon className="w-3.5 h-3.5 stroke-[1.6]" />
                    </IconButton>
                    <IconButton
                      onClick={() => handleMovePage(page, "down")}
                      disabled={index === document.pages.length - 1 || isBusy}
                      title="Move page down"
                      className="h-6 w-6"
                    >
                      <ChevronDownIcon className="w-3.5 h-3.5 stroke-[1.6]" />
                    </IconButton>
                    <IconButton
                      onClick={() => setRenamingPage(page)}
                      disabled={isBusy}
                      title="Rename page"
                      className="h-6 w-6"
                    >
                      <PencilIcon className="w-3.5 h-3.5 stroke-[1.6]" />
                    </IconButton>
                    <IconButton
                      onClick={() => setDeletingPage(page)}
                      disabled={document.pages.length <= 1 || isBusy}
                      title="Delete page"
                      className="h-6 w-6"
                    >
                      <TrashIcon className="w-3.5 h-3.5 stroke-[1.6]" />
                    </IconButton>
                  </div>
                </div>
              </ContextMenu.Trigger>
              <ContextMenu.Portal>
                <ContextMenu.Content className="min-w-48 bg-bg border border-border rounded-md shadow-lg py-1 z-50">
                  <ContextMenu.Item
                    className={menuItemClass}
                    disabled={isBusy}
                    onSelect={() => setRenamingPage(page)}
                  >
                    <PencilIcon className="w-4 h-4 stroke-[1.6]" />
                    Rename
                  </ContextMenu.Item>
                  <ContextMenu.Item
                    className={menuItemClass}
                    disabled={index === 0 || isBusy}
                    onSelect={() => handleMovePage(page, "up")}
                  >
                    <ArrowUpIcon className="w-4 h-4 stroke-[1.6]" />
                    Move Up
                  </ContextMenu.Item>
                  <ContextMenu.Item
                    className={menuItemClass}
                    disabled={index === document.pages.length - 1 || isBusy}
                    onSelect={() => handleMovePage(page, "down")}
                  >
                    <ChevronDownIcon className="w-4 h-4 stroke-[1.6]" />
                    Move Down
                  </ContextMenu.Item>
                  <ContextMenu.Separator className={menuSeparatorClass} />
                  <ContextMenu.Item
                    className={menuItemClass}
                    onSelect={() => handleCopyPageFilepath(page)}
                  >
                    <CopyIcon className="w-4 h-4 stroke-[1.6]" />
                    Copy Page Filepath
                  </ContextMenu.Item>
                  <ContextMenu.Item
                    className={menuItemClass}
                    onSelect={handleOpenDocumentFolder}
                  >
                    <FolderIcon className="w-4 h-4 stroke-[1.6]" />
                    Open Document Folder
                  </ContextMenu.Item>
                  <ContextMenu.Separator className={menuSeparatorClass} />
                  <ContextMenu.Item
                    className={
                      menuItemClass +
                      " text-red-500 hover:text-red-500 focus:text-red-500"
                    }
                    disabled={document.pages.length <= 1 || isBusy}
                    onSelect={() => setDeletingPage(page)}
                  >
                    <TrashIcon className="w-4 h-4 stroke-[1.6]" />
                    Delete Page
                  </ContextMenu.Item>
                </ContextMenu.Content>
              </ContextMenu.Portal>
            </ContextMenu.Root>
          );
        })}
      </div>

      <div className="border-t border-border p-2">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 mb-1"
          disabled={isBusy}
          onClick={onNormalizeDocument}
        >
          <RefreshCwIcon className="w-4 h-4 stroke-[1.6]" />
          Normalize
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2"
          disabled={isBusy}
          onClick={handleCreatePage}
        >
          <AddNoteIcon className="w-4 h-4 stroke-[1.6]" />
          New Page
        </Button>
      </div>

      <FolderNameDialog
        open={renamingPage !== null}
        onOpenChange={(open) => {
          if (!open) setRenamingPage(null);
        }}
        onConfirm={handleRenamePage}
        title="Rename page"
        description="Enter a new page title"
        confirmLabel="Rename"
        defaultValue={renamingPage?.title ?? ""}
      />

      <AlertDialog
        open={deletingPage !== null}
        onOpenChange={(open) => {
          if (!open) setDeletingPage(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete page?</AlertDialogTitle>
            <AlertDialogDescription>
              This will permanently delete this markdown page from the Document.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDeletePage}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </aside>
  );
}

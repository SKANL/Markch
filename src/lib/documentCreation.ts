import * as notesService from "../services/notes";

export function parentPath(path: string): string | undefined {
  const lastSlash = path.lastIndexOf("/");
  return lastSlash > 0 ? path.substring(0, lastSlash) : undefined;
}

export async function resolveNormalCreationParent(
  selectedNoteId?: string | null,
): Promise<string | undefined> {
  if (!selectedNoteId) return undefined;

  try {
    const document = await notesService.readDocumentForNote(selectedNoteId);
    if (document) {
      return parentPath(document.path);
    }
  } catch {
    // Fall back to the selected note's direct parent if document detection fails.
  }

  return parentPath(selectedNoteId);
}

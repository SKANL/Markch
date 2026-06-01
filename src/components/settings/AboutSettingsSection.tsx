import { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { showUpdateToast } from "../../App";
import { Button } from "../ui";
import { RefreshCwIcon, SpinnerIcon, GithubIcon } from "../icons";

export function AboutSettingsSection() {
  const [appVersion, setAppVersion] = useState<string>("");
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  const handleCheckForUpdates = async () => {
    setCheckingUpdate(true);
    const result = await showUpdateToast();
    setCheckingUpdate(false);
    if (result === "disabled") {
      toast.info("Updates are temporarily disabled for Markch.");
    } else if (result === "no-update") {
      toast.success("You're on the latest version!");
    } else if (result === "error") {
      toast.error("Could not check for updates. Try again later.");
    }
  };

  const handleOpenUrl = async (url: string) => {
    try {
      await invoke("open_url_safe", { url });
    } catch (err) {
      console.error("Failed to open URL:", err);
      toast.error(err instanceof Error ? err.message : "Failed to open URL");
    }
  };

  return (
    <div className="space-y-8 py-8">
      {/* Version */}
      <section className="pb-2">
        <h2 className="text-xl font-medium mb-0.5">Version</h2>
        <p className="text-sm text-text-muted mb-4">
          You are currently using Markch v{appVersion || "..."}
        </p>
        <Button
          onClick={handleCheckForUpdates}
          disabled={checkingUpdate}
          variant="outline"
          size="md"
          className="gap-1.25"
        >
          {checkingUpdate ? (
            <>
              <SpinnerIcon className="w-4.5 h-4.5 stroke-[1.5] animate-spin" />
              Checking...
            </>
          ) : (
            <>
              <RefreshCwIcon className="w-4.5 h-4.5 stroke-[1.5]" />
              Check for Updates
            </>
          )}
        </Button>
      </section>

      {/* Divider */}
      <div className="border-t border-border border-dashed" />

      {/* About Section */}
      <section className="pb-2">
        <h2 className="text-xl font-medium mb-1">About Markch</h2>
        <p className="text-sm text-text-muted mb-4">
          Markch is a minimalist markdown scratchpad for capturing quick
          thoughts, todos, and ideas. We're offline-first, keyboard-optimized,
          AI-compatible, and open source with no cloud, no accounts, and no
          subscriptions. Learn more on{" "}
          <button
            onClick={() => handleOpenUrl("https://github.com/SKANL/Markch")}
            className="text-text-muted border-b border-text-muted/50 hover:text-text hover:border-text cursor-pointer transition-colors"
          >
            GitHub
          </button>
          .
        </p>
        <p className="text-sm text-text-muted mb-4">
          Markch is based on{" "}
          <button
            onClick={() => handleOpenUrl("https://github.com/erictli/scratch")}
            className="text-text-muted border-b border-text-muted/50 hover:text-text hover:border-text cursor-pointer transition-colors"
          >
            Scratch
          </button>
          , an MIT-licensed app by Eric Li and contributors.
        </p>
        <div className="flex items-center gap-1">
          <Button
            onClick={() => handleOpenUrl("https://github.com/SKANL/Markch")}
            variant="outline"
            size="md"
            className="gap-1.25"
          >
            <GithubIcon className="w-4.5 h-4.5 stroke-[1.5]" />
            View on GitHub
          </Button>
          <Button
            onClick={() =>
              handleOpenUrl("https://github.com/SKANL/Markch/issues")
            }
            variant="ghost"
            size="md"
            className="gap-1.25 text-text"
          >
            Submit Feedback
          </Button>
        </div>
      </section>
    </div>
  );
}

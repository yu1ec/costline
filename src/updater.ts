import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check as checkForUpdate, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "current"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateProgress {
  downloaded: number;
  total: number;
}

export interface UpdateController {
  currentVersion: string;
  status: UpdateStatus;
  update: Update | null;
  progress: UpdateProgress | null;
  error: string | null;
  check: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
}

export function useUpdateController(): UpdateController {
  const [currentVersion, setCurrentVersion] = useState("");
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const checkingRef = useRef(false);
  const updateRef = useRef<Update | null>(null);

  useEffect(() => {
    let disposed = false;
    getVersion()
      .then((version) => {
        if (!disposed) setCurrentVersion(version);
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(
    () => () => {
      void updateRef.current?.close().catch(() => {});
      updateRef.current = null;
    },
    [],
  );

  const check = useCallback(async (silent = false) => {
    if (checkingRef.current) return;
    checkingRef.current = true;
    if (!silent) {
      setStatus("checking");
      setError(null);
    }
    try {
      const next = await checkForUpdate();
      await updateRef.current?.close().catch(() => {});
      updateRef.current = next;
      setUpdate(next);
      setStatus(next ? "available" : "current");
    } catch (cause) {
      if (!silent) {
        setError(errorMessage(cause));
        setStatus("error");
      }
    } finally {
      checkingRef.current = false;
    }
  }, []);

  const install = useCallback(async () => {
    const target = updateRef.current;
    if (!target) return;
    setError(null);
    setStatus("downloading");
    setProgress({ downloaded: 0, total: 0 });
    try {
      await target.downloadAndInstall((event) => {
        if (event.event === "Started") {
          setProgress({ downloaded: 0, total: event.data.contentLength ?? 0 });
        } else if (event.event === "Progress") {
          setProgress((current) => ({
            downloaded: (current?.downloaded ?? 0) + event.data.chunkLength,
            total: current?.total ?? 0,
          }));
        }
      });
      setStatus("ready");
      await relaunch();
    } catch (cause) {
      setError(errorMessage(cause));
      setStatus("error");
    }
  }, []);

  return {
    currentVersion,
    status,
    update,
    progress,
    error,
    check,
    install,
  };
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

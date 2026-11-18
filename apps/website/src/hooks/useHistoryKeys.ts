import { useEffect } from "react";

interface HistoryKeys {
  length: number;
  viewIndex: number;
  onChange: (index: number) => void;
}

export function useHistoryKeys({ length, viewIndex, onChange }: HistoryKeys) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target?.closest(
          "input, textarea, select, [contenteditable='true'], [role='dialog']",
        )
      ) {
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      const last = Math.max(0, length - 1);

      switch (event.key) {
        case "ArrowLeft":
          event.preventDefault();
          onChange(Math.max(0, viewIndex - 1));
          break;
        case "ArrowRight":
          event.preventDefault();
          onChange(Math.min(last, viewIndex + 1));
          break;
        case "Home":
          event.preventDefault();
          onChange(0);
          break;
        case "End":
          event.preventDefault();
          onChange(last);
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [length, onChange, viewIndex]);
}

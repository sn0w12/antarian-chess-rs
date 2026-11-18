import { Spinner } from "@/components/ui/spinner";

export function GameLoading({
  ready,
  error,
}: {
  ready: boolean;
  error: string | null;
}) {
  if (ready && !error) {
    return null;
  }

  return (
    <div className="flex h-dvh flex-col items-center justify-center overflow-hidden gap-3 px-6 text-center">
      {error ? (
        <>
          <p className="text-sm font-medium">Could not load the engine</p>
          <p className="max-w-md text-sm text-muted-foreground">{error}</p>
        </>
      ) : (
        <>
          <Spinner aria-label="Loading engine" />
          <p className="text-sm text-muted-foreground">Loading the engine…</p>
        </>
      )}
    </div>
  );
}

interface GoButtonProps {
  disabled?: boolean;
  loading?: boolean;
  onGo?: () => void;
}

export function GoButton({
  disabled = false,
  loading = false,
  onGo,
}: GoButtonProps) {
  const isDisabled = disabled || loading;

  return (
    <button
      type="button"
      disabled={isDisabled}
      onClick={onGo}
      className={[
        "flex items-center justify-center gap-2 rounded-md px-5 py-2.5 text-sm font-bold tracking-wide transition-all",
        isDisabled
          ? "cursor-not-allowed bg-zinc-800 text-zinc-600 opacity-60"
          : "bg-green-600 text-white hover:bg-green-500 active:scale-[0.98]",
      ].join(" ")}
    >
      {loading && (
        <span
          aria-hidden
          className="h-4 w-4 animate-spin rounded-full border-2 border-white/40 border-t-white"
        />
      )}
      <span>{loading ? "Preparando..." : "GO!"}</span>
    </button>
  );
}
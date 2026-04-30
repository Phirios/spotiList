export function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wider text-zinc-500 mb-1">
        {label}
      </div>
      <div>{children}</div>
    </div>
  );
}

interface ThemeToggleProps {
  dark: boolean;
  onToggle: () => void;
}

export function ThemeToggle({ dark, onToggle }: ThemeToggleProps) {
  return (
    <button
      className="subtle-button theme-toggle"
      type="button"
      aria-label={dark ? "Switch to light mode" : "Switch to dark mode"}
      title={dark ? "Switch to light mode" : "Switch to dark mode"}
      onClick={onToggle}
    >
      <span
        className={`theme-toggle-icon ${dark ? "theme-toggle-icon-light" : "theme-toggle-icon-dark"}`}
        aria-hidden="true"
      />
      <span>{dark ? "Light mode" : "Dark mode"}</span>
    </button>
  );
}

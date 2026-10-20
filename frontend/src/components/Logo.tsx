import React from "react";

interface LogoProps {
  isCollapsed: boolean;
}

/** Simple microphone mark used as the app logo. */
export function MicMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="9" y="2" width="6" height="11" rx="3" />
      <path d="M5 11a7 7 0 0 0 14 0" />
      <line x1="12" y1="18" x2="12" y2="21" />
      <line x1="8.5" y1="21" x2="15.5" y2="21" />
    </svg>
  );
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(({ isCollapsed }, ref) => {
  if (isCollapsed) {
    return (
      <button
        ref={ref}
        type="button"
        aria-label="Intake Pro"
        className="flex items-center justify-center mb-2 bg-transparent border-none p-0 hover:opacity-80 transition-opacity"
      >
        <MicMark className="w-7 h-7 text-blue-600" />
      </button>
    );
  }

  return (
    <div className="flex items-center justify-center gap-2 w-full mb-2" aria-label="Intake Pro">
      <MicMark className="w-5 h-5 text-blue-600" />
      <span className="text-lg font-semibold text-gray-700">Intake Pro</span>
    </div>
  );
});

Logo.displayName = "Logo";

export default Logo;

import type { LaunchReport } from "../types";

const STATUS_ICON: Record<string, string> = {
  started: "✅",
  failed: "❌",
  skipped: "⏭️",
};

export function LaunchProgress({ report }: { report: LaunchReport }) {
  return (
    <div className="launch-progress">
      <h3>Launch result</h3>
      <ul>
        {report.outcomes.map((outcome) => (
          <li key={outcome.actionId} className={`outcome outcome-${outcome.status}`}>
            <span className="outcome-icon">{STATUS_ICON[outcome.status] ?? "•"}</span>
            <span className="outcome-label">{outcome.label}</span>
            {outcome.message && <span className="outcome-message">{outcome.message}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}

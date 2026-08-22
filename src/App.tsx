import { useEffect, useState } from "react";
import { api } from "./api";
import type { ConfigStatus, Workspace } from "./types";
import { WorkspaceList } from "./components/WorkspaceList";
import { WorkspaceEditor } from "./components/WorkspaceEditor";
import { ConfigWarning } from "./components/ConfigWarning";
import { DiagnosticsFooter } from "./components/DiagnosticsFooter";
import "./App.css";

type View = { name: "list" } | { name: "edit"; workspaceId: string | null };

function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [view, setView] = useState<View>({ name: "list" });
  const [configStatus, setConfigStatus] = useState<ConfigStatus | null>(null);

  async function refresh() {
    setWorkspaces(await api.listWorkspaces());
  }

  useEffect(() => {
    refresh().catch(console.error);
    api.configStatus().then(setConfigStatus).catch(console.error);
  }, []);

  return (
    <main className="container">
      <h1>Click</h1>
      {configStatus && <ConfigWarning status={configStatus} />}
      {view.name === "list" ? (
        <WorkspaceList
          workspaces={workspaces}
          onEdit={(id) => setView({ name: "edit", workspaceId: id })}
          onNew={() => setView({ name: "edit", workspaceId: null })}
          onChanged={refresh}
        />
      ) : (
        <WorkspaceEditor
          workspaceId={view.workspaceId}
          onSaved={async () => {
            await refresh();
            setView({ name: "list" });
          }}
          onCancel={() => setView({ name: "list" })}
        />
      )}
      <DiagnosticsFooter />
    </main>
  );
}

export default App;

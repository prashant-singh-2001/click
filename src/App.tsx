import { useEffect, useState } from "react";
import { api } from "./api";
import { newWorkspace } from "./types";
import type { ConfigStatus, Workspace } from "./types";
import { WorkspaceList } from "./components/WorkspaceList";
import { WorkspaceEditor } from "./components/WorkspaceEditor";
import { ConfigWarning } from "./components/ConfigWarning";
import "./App.css";

type View = { name: "list" } | { name: "edit"; workspace: Workspace };

function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [view, setView] = useState<View>({ name: "list" });
  const [configStatus, setConfigStatus] = useState<ConfigStatus | null>(null);

  async function refresh() {
    setWorkspaces(await api.listWorkspaces());
  }

  useEffect(() => {
    refresh();
    api.configStatus().then(setConfigStatus);
  }, []);

  return (
    <main className="container">
      <h1>Click</h1>
      {configStatus && <ConfigWarning status={configStatus} />}
      {view.name === "list" ? (
        <WorkspaceList
          workspaces={workspaces}
          onEdit={(workspace) => setView({ name: "edit", workspace })}
          onNew={() => setView({ name: "edit", workspace: newWorkspace() })}
          onDeleted={refresh}
        />
      ) : (
        <WorkspaceEditor
          workspace={view.workspace}
          onSaved={async () => {
            await refresh();
            setView({ name: "list" });
          }}
          onCancel={() => setView({ name: "list" })}
        />
      )}
    </main>
  );
}

export default App;

import { useEffect, useState } from "react";
import { api } from "./api";
import { newWorkspace } from "./types";
import type { Workspace } from "./types";
import { WorkspaceList } from "./components/WorkspaceList";
import { WorkspaceEditor } from "./components/WorkspaceEditor";
import "./App.css";

type View = { name: "list" } | { name: "edit"; workspace: Workspace };

function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [view, setView] = useState<View>({ name: "list" });

  async function refresh() {
    setWorkspaces(await api.listWorkspaces());
  }

  useEffect(() => {
    refresh();
  }, []);

  return (
    <main className="container">
      <h1>Click</h1>
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

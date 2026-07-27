import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type ModelAssignment = {
  provider: string;
  model: string;
  endpoint: string | null;
};

type TeamLeader = {
  id: string;
  display_name: string;
  professional_role: string;
  department: string;
  model: ModelAssignment;
};

type CrewManifest = {
  command_model: ModelAssignment | null;
  leaders: TeamLeader[];
};

type WarpCoreStatus = {
  queued: number;
  transmitting: number;
  retry: number;
  acknowledged: number;
  rejected: number;
  dirty_documents: number;
};

type SavedCommand = {
  command_id: string;
};

type ProjectConnection = {
  adapter: string;
  display_name: string;
  repository: string;
  workspace_path: string | null;
  default_branch: string;
};

type Scene = "bridge" | "plan" | "engineering" | "crew";

const isNative = "__TAURI_INTERNALS__" in window;
let activeCrew: CrewManifest = {
  command_model: {
    provider: "provider-a",
    model: "command-model",
    endpoint: null,
  },
  leaders: [
    leader("riker", "William Riker", "Agent Operations Director", "Command", "provider-a", "orchestration-model"),
    leader("data", "Data", "Principal Analyst", "Research & Analysis", "provider-b", "reasoning-model"),
    leader("la-forge", "Geordi La Forge", "Principal Software Engineer", "Engineering", "provider-c", "coding-model"),
    leader("worf", "Worf", "Security & Risk Director", "Security", "provider-b", "security-model"),
    leader("troi", "Deanna Troi", "Organisational Psychologist", "People & Users", "provider-a", "human-context-model"),
    leader("crusher", "Beverly Crusher", "Quality & Safety Director", "Quality", "provider-b", "diagnostic-model"),
  ],
};

function leader(
  id: string,
  displayName: string,
  professionalRole: string,
  department: string,
  provider: string,
  model: string,
): TeamLeader {
  return {
    id,
    display_name: displayName,
    professional_role: professionalRole,
    department,
    model: { provider, model, endpoint: null },
  };
}

function setText(selector: string, text: string) {
  const element = document.querySelector(selector);
  if (element) element.textContent = text;
}

function setScene(name: Scene) {
  const isPlanMode = name === "plan";
  const isEngineering = name === "engineering";
  const isCrew = name === "crew";
  document.body.dataset.scene = name;
  setText(
    "#scene-code",
    isPlanMode
      ? "CONFERENCE / 02"
      : isEngineering
        ? "ENGINEERING / 03"
        : isCrew
          ? "CREW / 04"
          : "BRIDGE / 01",
  );
  setText(
    "#scene-title",
    isPlanMode
      ? "Plan Mode"
      : isEngineering
        ? "Bug Resolution"
        : isCrew
          ? "Commissioning"
          : "Senior Staff",
  );
  setText(
    "#scene-description",
    isPlanMode
      ? "Convene the right perspectives before committing to a plan."
      : isEngineering
        ? "Reproduce, isolate, patch, and verify with a focused technical team."
        : isCrew
          ? "Connect a project. Assign the right model to every department."
          : "Independent models. One accountable command structure.",
  );
  setText("#table-mode", isEngineering ? "ENGINEERING ROOM" : "PLAN MODE");
  setText("#table-title", isEngineering ? "BUG RESOLUTION" : "PLANNING SESSION");
  setText(
    "#table-subtitle",
    isEngineering ? "REPRODUCE · ISOLATE · PATCH · VERIFY" : "6 INDEPENDENT PERSPECTIVES",
  );
  if (commandInput) {
    commandInput.placeholder = isPlanMode
      ? "Give the planning team a problem to work through…"
      : isEngineering
        ? "Describe the bug, symptoms, and expected behaviour…"
        : isCrew
          ? "Crew configuration does not dispatch commands."
          : "Give the senior staff an objective…";
  }
  document.querySelectorAll<HTMLButtonElement>(".nav[data-scene]").forEach((button) => {
    button.classList.toggle("active", button.dataset.scene === name);
  });
}

document.querySelectorAll<HTMLButtonElement>("[data-scene]").forEach((button) => {
  button.addEventListener("click", () => {
    setScene(button.dataset.scene as Scene);
  });
});

const commandForm = document.querySelector<HTMLFormElement>("#command-form");
const commandInput = document.querySelector<HTMLInputElement>("#command-input");
const commandTranscript = document.querySelector<HTMLElement>("#command-transcript");
const projectForm = document.querySelector<HTMLFormElement>("#project-form");
const projectAdapterInput = document.querySelector<HTMLSelectElement>("#project-adapter-input");
const projectNameInput = document.querySelector<HTMLInputElement>("#project-name-input");
const projectRepositoryInput = document.querySelector<HTMLInputElement>("#project-repository-input");
const projectWorkspaceInput = document.querySelector<HTMLInputElement>("#project-workspace-input");
const projectBranchInput = document.querySelector<HTMLInputElement>("#project-branch-input");
const commandModelForm = document.querySelector<HTMLFormElement>("#command-model-form");
const commandProviderInput = document.querySelector<HTMLInputElement>("#command-provider-input");
const commandModelInput = document.querySelector<HTMLInputElement>("#command-model-input");
const commandEndpointInput = document.querySelector<HTMLInputElement>("#command-endpoint-input");
const staffingForm = document.querySelector<HTMLFormElement>("#staffing-form");
const staffingPrompt = document.querySelector<HTMLTextAreaElement>("#staffing-prompt");

commandForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const message = commandInput?.value.trim() ?? "";
  if (!message || !commandInput || !commandTranscript) return;

  commandInput.disabled = true;
  const submitButton = commandForm.querySelector<HTMLButtonElement>("button");
  if (submitButton) submitButton.disabled = true;

  try {
    if (isNative) {
      await invoke<SavedCommand>("submit_captain_message", {
        messageId: crypto.randomUUID(),
        text: message,
      });
      commandTranscript.innerHTML = `<b>PICARD</b><span>${escapeHtml(message)}</span>`;
      setText("#system-status", "COMMAND SAVED · AWAITING FIRST OFFICER");
    } else {
      commandTranscript.innerHTML =
        `<b>PREVIEW</b><span>${escapeHtml(message)} · Native Warp Core required to dispatch.</span>`;
    }
    commandInput.value = "";
  } catch (error) {
    setText("#system-status", `COMMAND REJECTED · ${String(error)}`);
  } finally {
    commandInput.disabled = false;
    if (submitButton) submitButton.disabled = false;
    commandInput.focus();
  }
});

projectForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (
    !projectAdapterInput ||
    !projectNameInput ||
    !projectRepositoryInput ||
    !projectWorkspaceInput ||
    !projectBranchInput
  ) {
    return;
  }

  const connection: ProjectConnection = {
    adapter: projectAdapterInput.value,
    display_name: projectNameInput.value.trim(),
    repository: projectRepositoryInput.value.trim(),
    workspace_path: projectWorkspaceInput.value.trim() || null,
    default_branch: projectBranchInput.value.trim(),
  };
  const submitButton = projectForm.querySelector<HTMLButtonElement>("button[type='submit']");
  if (submitButton) submitButton.disabled = true;

  try {
    const saved = isNative
      ? await invoke<ProjectConnection>("connect_project", { connection })
      : connection;
    renderProjectConnection(saved);
    setText(
      "#system-status",
      isNative
        ? "PROJECT COMMISSIONED · SAVED THROUGH WARP CORE"
        : "VISUAL PREVIEW · PROJECT CONNECTION SIMULATED",
    );
  } catch (error) {
    setText("#system-status", `PROJECT REJECTED · ${String(error)}`);
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
});

commandModelForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!commandProviderInput || !commandModelInput || !commandEndpointInput) return;

  const model: ModelAssignment = {
    provider: commandProviderInput.value.trim(),
    model: commandModelInput.value.trim(),
    endpoint: commandEndpointInput.value.trim() || null,
  };
  const submit = commandModelForm.querySelector<HTMLButtonElement>("button[type='submit']");
  if (submit) submit.disabled = true;

  try {
    if (isNative) {
      activeCrew = await invoke<CrewManifest>("assign_command_model", { model });
    } else {
      activeCrew = { ...activeCrew, command_model: model };
    }
    renderCommandModel(activeCrew);
    setText("#system-status", "PICARD · COMMAND MODEL CONNECTED");
  } catch (error) {
    setText("#system-status", `COMMAND MODEL REJECTED · ${String(error)}`);
  } finally {
    if (submit) submit.disabled = false;
  }
});

staffingForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = staffingPrompt?.value.trim() ?? "";
  if (!prompt || !staffingPrompt) return;
  if (!activeCrew.command_model) {
    setText("#system-status", "CONNECT THE COMMAND MODEL BEFORE SUBMITTING A STAFFING BRIEF");
    commandProviderInput?.focus();
    return;
  }

  const submit = staffingForm.querySelector<HTMLButtonElement>("button[type='submit']");
  if (submit) submit.disabled = true;
  try {
    if (isNative) {
      await invoke<SavedCommand>("submit_staffing_brief", {
        briefId: crypto.randomUUID(),
        prompt,
      });
    }
    setText(
      "#system-status",
      isNative
        ? "STAFFING BRIEF SAVED · COMMAND MODEL QUEUED"
        : "VISUAL PREVIEW · STAFFING BRIEF READY",
    );
  } catch (error) {
    setText("#system-status", `STAFFING BRIEF REJECTED · ${String(error)}`);
  } finally {
    if (submit) submit.disabled = false;
  }
});

function escapeHtml(value: string) {
  const span = document.createElement("span");
  span.textContent = value;
  return span.innerHTML;
}

async function connectWarpCore() {
  if (!isNative) {
    renderCrew(activeCrew);
    setText("#system-status", "VISUAL PREVIEW · WARP CORE STANDBY");
    return;
  }

  try {
    const [crew, status, project] = await Promise.all([
      invoke<CrewManifest>("get_crew_manifest"),
      invoke<WarpCoreStatus>("get_warp_core_status"),
      invoke<ProjectConnection | null>("get_project_connection"),
    ]);
    activeCrew = crew;
    setText(
      "#system-status",
      `WARP CORE ONLINE · ${crew.leaders.length} OFFICERS · ${status.queued + status.retry} TO BASE`,
    );
    setText("#metric-core", "ONLINE");
    setText("#metric-officers", String(crew.leaders.length).padStart(2, "0"));
    setText("#metric-queue", String(status.queued + status.retry).padStart(2, "0"));
    renderCrew(crew);
    if (project) renderProjectConnection(project);
  } catch (error) {
    setText("#system-status", `WARP CORE OFFLINE · ${String(error)}`);
  }
}

function renderCrew(crew: CrewManifest) {
  renderCommandModel(crew);
  renderModelAssignments(crew);
  renderModelSettings(crew);
}

function renderCommandModel(crew: CrewManifest) {
  if (!crew.command_model) return;
  if (commandProviderInput) commandProviderInput.value = crew.command_model.provider;
  if (commandModelInput) commandModelInput.value = crew.command_model.model;
  if (commandEndpointInput) commandEndpointInput.value = crew.command_model.endpoint ?? "";
}

function renderModelAssignments(crew: CrewManifest) {
  const list = document.querySelector<HTMLElement>("#crew-models");
  if (!list) return;
  list.replaceChildren(
    ...crew.leaders.slice(0, 6).map((leader) => {
      const row = document.createElement("div");
      const dot = document.createElement("i");
      dot.className = `dot ${departmentDotClass(leader.department)}`;
      const name = document.createElement("strong");
      name.textContent = leader.display_name.split(" ").at(-1) ?? leader.display_name;
      const model = document.createElement("span");
      model.textContent = leader.model.model;
      row.append(dot, name, model);
      return row;
    }),
  );
}

function renderModelSettings(crew: CrewManifest) {
  const settings = document.querySelector<HTMLElement>("#model-settings");
  if (!settings) return;

  settings.replaceChildren(
    ...crew.leaders.map((crewLeader) => {
      const form = document.createElement("form");
      form.className = "model-card";
      form.dataset.leaderId = crewLeader.id;

      const heading = document.createElement("div");
      heading.className = "model-card-heading";
      const dot = document.createElement("i");
      dot.className = `dot ${departmentDotClass(crewLeader.department)}`;
      const identity = document.createElement("span");
      const name = document.createElement("strong");
      name.textContent = crewLeader.display_name;
      const role = document.createElement("small");
      role.textContent = crewLeader.professional_role;
      identity.append(name, role);
      heading.append(dot, identity);

      const provider = modelField("PROVIDER", "provider", crewLeader.model.provider);
      const model = modelField("MODEL", "model", crewLeader.model.model);
      const save = document.createElement("button");
      save.type = "submit";
      save.textContent = "ASSIGN";
      form.append(heading, provider, model, save);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        void assignModel(form, crewLeader);
      });
      return form;
    }),
  );
}

function modelField(labelText: string, name: string, value: string) {
  const label = document.createElement("label");
  const caption = document.createElement("span");
  caption.textContent = labelText;
  const input = document.createElement("input");
  input.name = name;
  input.value = value;
  input.required = true;
  label.append(caption, input);
  return label;
}

async function assignModel(form: HTMLFormElement, crewLeader: TeamLeader) {
  const provider = form.elements.namedItem("provider") as HTMLInputElement | null;
  const model = form.elements.namedItem("model") as HTMLInputElement | null;
  const submit = form.querySelector<HTMLButtonElement>("button[type='submit']");
  if (!provider?.value.trim() || !model?.value.trim()) return;

  if (submit) submit.disabled = true;
  try {
    const assignment: ModelAssignment = {
      provider: provider.value.trim(),
      model: model.value.trim(),
      endpoint: crewLeader.model.endpoint,
    };
    if (isNative) {
      activeCrew = await invoke<CrewManifest>("assign_leader_model", {
        leaderId: crewLeader.id,
        model: assignment,
      });
    } else {
      activeCrew = {
        ...activeCrew,
        leaders: activeCrew.leaders.map((leaderEntry) =>
          leaderEntry.id === crewLeader.id
            ? { ...leaderEntry, model: assignment }
            : leaderEntry,
        ),
      };
    }
    renderCrew(activeCrew);
    setText(
      "#system-status",
      `${crewLeader.display_name.toUpperCase()} · MODEL ASSIGNMENT SAVED`,
    );
  } catch (error) {
    setText("#system-status", `MODEL ASSIGNMENT REJECTED · ${String(error)}`);
    if (submit) submit.disabled = false;
  }
}

function renderProjectConnection(connection: ProjectConnection) {
  setText("#project-adapter", connection.adapter.toUpperCase());
  setText("#project-name", connection.display_name.toUpperCase());
  setText(
    "#project-summary",
    `${connection.repository} · ${connection.default_branch}`,
  );

  if (projectAdapterInput) projectAdapterInput.value = connection.adapter;
  if (projectNameInput) projectNameInput.value = connection.display_name;
  if (projectRepositoryInput) projectRepositoryInput.value = connection.repository;
  if (projectWorkspaceInput) projectWorkspaceInput.value = connection.workspace_path ?? "";
  if (projectBranchInput) projectBranchInput.value = connection.default_branch;
}

function departmentDotClass(department: string) {
  if (department === "Command") return "command-dot";
  if (department === "Engineering") return "engineering-dot";
  if (department === "Security") return "security-dot";
  if (department === "Research & Analysis") return "science-dot";
  if (department === "Quality") return "medical-dot";
  return "people-dot";
}

void connectWarpCore();

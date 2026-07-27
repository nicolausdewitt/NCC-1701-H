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
  access: "read_only" | "read_write";
  credential_profile: string | null;
};

type GithubWriteAuthorization = {
  connection: ProjectConnection;
  account: string;
  permission: string;
};

type OpenAiConnection = {
  provider: string;
  adapter: string;
  auth_method: string;
  credential_profile: string;
  status: string;
};

type Scene = "bridge" | "plan" | "engineering" | "crew";

const isNative = "__TAURI_INTERNALS__" in window;
let openAiConnection: OpenAiConnection | null = null;
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
const projectConnectButton = document.querySelector<HTMLButtonElement>("#project-connect-button");
const projectConnectionResult = document.querySelector<HTMLElement>("#project-connection-result");
const projectResultTitle = document.querySelector<HTMLElement>("#project-result-title");
const projectResultDetail = document.querySelector<HTMLElement>("#project-result-detail");
const projectStepState = document.querySelector<HTMLElement>("#project-step-state");
const githubAuthorizeButton = document.querySelector<HTMLButtonElement>("#github-authorize-button");
const commandModelSettings = document.querySelector<HTMLElement>(".command-model-settings");
const openAiConnectButton = document.querySelector<HTMLButtonElement>("#openai-connect-button");
const openAiConnectionResult = document.querySelector<HTMLElement>("#openai-connection-result");
const openAiResultTitle = document.querySelector<HTMLElement>("#openai-result-title");
const openAiResultDetail = document.querySelector<HTMLElement>("#openai-result-detail");
const openAiModelLabel = document.querySelector<HTMLElement>("#openai-model-label");
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
    access: "read_only",
    credential_profile: null,
  };
  if (projectConnectButton) {
    projectConnectButton.disabled = true;
    projectConnectButton.textContent = "CONTACTING ADAPTER…";
  }
  setProjectConnectionFeedback("connecting", "CONTACTING ADAPTER", connection.adapter.toUpperCase());
  const connectionStartedAt = performance.now();

  try {
    const saved = isNative
      ? await invoke<ProjectConnection>("connect_project", { connection })
      : connection;
    await holdFeedbackFor(connectionStartedAt, 500);
    renderProjectConnection(saved);
    setProjectConnectionFeedback(
      isNative ? "connected" : "preview",
      saved.adapter === "github" ? "READ ONLY" : isNative ? "GIT TARGET SAVED" : "PREVIEW CONFIGURED",
      saved.adapter === "github"
        ? "AUTHORISE BEFORE CHANGES"
        : isNative
          ? `${saved.adapter.toUpperCase()} · ${saved.default_branch}`
          : "NATIVE APP PERSISTS IT",
    );
    showGithubWriteOffer(saved);
    if (projectConnectButton) {
      projectConnectButton.textContent = isNative ? "UPDATE PROJECT" : "CONFIGURED FOR PREVIEW";
    }
    if (projectStepState) {
      projectStepState.textContent = isNative ? "WARP CORE SAVED" : "PREVIEW ONLY";
    }
    commandModelSettings?.classList.add("next-step");
    setText(
      "#system-status",
      isNative
        ? "PROJECT COMMISSIONED · SAVED THROUGH WARP CORE"
        : "VISUAL PREVIEW · PROJECT CONNECTION SIMULATED",
    );
  } catch (error) {
    const message = String(error);
    setProjectConnectionFeedback("error", "CONNECTION REJECTED", message);
    if (projectConnectButton) projectConnectButton.textContent = "RETRY CONNECTION";
    if (projectStepState) projectStepState.textContent = "ACTION REQUIRED";
    setText("#system-status", `PROJECT REJECTED · ${String(error)}`);
  } finally {
    if (projectConnectButton) projectConnectButton.disabled = false;
  }
});

openAiConnectButton?.addEventListener("click", async () => {
  openAiConnectButton.disabled = true;
  openAiConnectButton.textContent = isNative ? "OPENING SIGN-IN…" : "CHECKING NATIVE APP…";
  setOpenAiConnectionFeedback(
    "connecting",
    isNative ? "CONTACTING OPENAI" : "NATIVE HANDOFF REQUIRED",
    isNative ? "CHECKING CODEX SESSION" : "SIGN-IN RUNS OUTSIDE THE WEBVIEW",
  );
  const startedAt = performance.now();

  if (!isNative) {
    await holdFeedbackFor(startedAt, 600);
    setOpenAiConnectionFeedback(
      "preview",
      "NATIVE APP REQUIRED",
      "OPEN NCC TO SIGN IN WITH CHATGPT",
    );
    openAiConnectButton.textContent = "CONNECT OPENAI";
    openAiConnectButton.disabled = false;
    return;
  }

  try {
    openAiConnection = await invoke<OpenAiConnection>("authorize_openai");
    const model: ModelAssignment = {
      provider: "openai-codex",
      model: "codex-default",
      endpoint: null,
    };
    activeCrew = await invoke<CrewManifest>("assign_command_model", { model });
    await holdFeedbackFor(startedAt, 600);
    renderOpenAiConnection(openAiConnection);
    renderCommandModel(activeCrew);
    setText("#system-status", "OPENAI CONNECTED · PICARD COMMAND MODEL READY");
  } catch (error) {
    setOpenAiConnectionFeedback("error", "OPENAI SIGN-IN FAILED", String(error));
    openAiConnectButton.textContent = "RETRY OPENAI";
    setText("#system-status", `OPENAI SIGN-IN FAILED · ${String(error)}`);
  } finally {
    openAiConnectButton.disabled = false;
  }
});

staffingForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = staffingPrompt?.value.trim() ?? "";
  if (!prompt || !staffingPrompt) return;
  if (!openAiConnection) {
    setText("#system-status", "CONNECT OPENAI BEFORE SUBMITTING A STAFFING BRIEF");
    openAiConnectButton?.focus();
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
    const [crew, status, project, openAi] = await Promise.all([
      invoke<CrewManifest>("get_crew_manifest"),
      invoke<WarpCoreStatus>("get_warp_core_status"),
      invoke<ProjectConnection | null>("get_project_connection"),
      invoke<OpenAiConnection | null>("get_openai_connection"),
    ]);
    activeCrew = crew;
    openAiConnection = openAi;
    setText(
      "#system-status",
      `WARP CORE ONLINE · ${crew.leaders.length} OFFICERS · ${status.queued + status.retry} TO BASE`,
    );
    setText("#metric-core", "ONLINE");
    setText("#metric-officers", String(crew.leaders.length).padStart(2, "0"));
    setText("#metric-queue", String(status.queued + status.retry).padStart(2, "0"));
    renderCrew(crew);
    if (project) {
      renderProjectConnection(project);
      setProjectConnectionFeedback(
        "connected",
        project.adapter === "github" && project.access === "read_only"
          ? "READ ONLY"
          : "GIT TARGET SAVED",
        project.adapter === "github" && project.access === "read_only"
          ? "AUTHORISE BEFORE CHANGES"
          : `${project.adapter.toUpperCase()} · ${project.default_branch}`,
      );
      showGithubWriteOffer(project);
      if (projectConnectButton) projectConnectButton.textContent = "UPDATE PROJECT";
      if (projectStepState) projectStepState.textContent = "WARP CORE SAVED";
    }
    if (openAi) renderOpenAiConnection(openAi);
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
  if (!openAiConnection) {
    if (openAiModelLabel) openAiModelLabel.textContent = "MODEL · AFTER SIGN-IN";
    return;
  }
  if (!crew.command_model) return;
  if (openAiModelLabel) {
    openAiModelLabel.textContent =
      crew.command_model.model === "codex-default"
        ? "MODEL · AUTO"
        : `MODEL · ${crew.command_model.model.toUpperCase()}`;
  }
}

function renderOpenAiConnection(connection: OpenAiConnection) {
  const method = connection.auth_method === "chatgpt" ? "CHATGPT" : "API KEY";
  setOpenAiConnectionFeedback("connected", "OPENAI CONNECTED", `${method} · CODEX SESSION`);
  if (openAiConnectButton) openAiConnectButton.textContent = "OPENAI CONNECTED";
  commandModelSettings?.classList.remove("next-step");
}

function setOpenAiConnectionFeedback(
  state: "idle" | "connecting" | "connected" | "preview" | "error",
  title: string,
  detail: string,
) {
  if (openAiConnectionResult) openAiConnectionResult.dataset.state = state;
  if (openAiResultTitle) openAiResultTitle.textContent = title;
  if (openAiResultDetail) openAiResultDetail.textContent = detail;
  if (state === "connected" || state === "preview" || state === "error") {
    pulseRefresh(openAiConnectionResult);
  }
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
    `${connection.repository} · ${connection.default_branch} · ${connection.access === "read_write" ? "WRITE ENABLED" : "READ ONLY"}`,
  );

  if (projectAdapterInput) projectAdapterInput.value = connection.adapter;
  if (projectNameInput) projectNameInput.value = connection.display_name;
  if (projectRepositoryInput) projectRepositoryInput.value = connection.repository;
  if (projectWorkspaceInput) projectWorkspaceInput.value = connection.workspace_path ?? "";
  if (projectBranchInput) projectBranchInput.value = connection.default_branch;

  pulseRefresh(
    document.querySelector<HTMLElement>("#project-name"),
    document.querySelector<HTMLElement>("#project-adapter"),
  );
}

function showGithubWriteOffer(connection: ProjectConnection) {
  const shouldOffer = connection.adapter === "github" && connection.access === "read_only";
  if (githubAuthorizeButton) {
    githubAuthorizeButton.hidden = !shouldOffer;
    githubAuthorizeButton.textContent = "AUTHORISE";
  }
  projectConnectionResult?.classList.toggle("has-action", shouldOffer);
}

githubAuthorizeButton?.addEventListener("click", async () => {
  githubAuthorizeButton.hidden = true;
  projectConnectionResult?.classList.remove("has-action");

  if (!isNative) {
    setProjectConnectionFeedback(
      "preview",
      "NATIVE AUTH REQUIRED",
      "OPEN NCC TO AUTHORISE VIA GITHUB",
    );
    return;
  }

  setProjectConnectionFeedback("connecting", "OPENING GITHUB", "COMPLETE BROWSER AUTHORISATION");
  setText("#system-status", "WAITING FOR GITHUB AUTHORISATION");
  try {
    const authorization = await invoke<GithubWriteAuthorization>("authorize_github_writes");
    renderProjectConnection(authorization.connection);
    showGithubWriteOffer(authorization.connection);
    setProjectConnectionFeedback(
      "connected",
      "WRITE ENABLED",
      `${authorization.account.toUpperCase()} · ${authorization.permission}`,
    );
    if (projectStepState) projectStepState.textContent = "GITHUB API AUTHORISED";
    setText(
      "#system-status",
      `GITHUB API CONNECTED · ${authorization.account.toUpperCase()} · ${authorization.permission}`,
    );
  } catch (error) {
    setProjectConnectionFeedback("error", "GITHUB AUTH FAILED", String(error));
    githubAuthorizeButton.hidden = false;
    githubAuthorizeButton.textContent = "RETRY AUTH";
    projectConnectionResult?.classList.add("has-action");
    setText("#system-status", `GITHUB AUTHORISATION FAILED · ${String(error)}`);
  }
});

function setProjectConnectionFeedback(
  state: "idle" | "connecting" | "connected" | "preview" | "error",
  title: string,
  detail: string,
) {
  if (projectConnectionResult) projectConnectionResult.dataset.state = state;
  if (projectResultTitle) projectResultTitle.textContent = title;
  if (projectResultDetail) projectResultDetail.textContent = detail;
  if (state === "connected" || state === "preview" || state === "error") {
    pulseRefresh(projectConnectionResult);
  }
}

function pulseRefresh(...elements: Array<HTMLElement | null>) {
  for (const element of elements) {
    if (!element) continue;
    element.classList.remove("data-refresh-pulse");
    void element.offsetWidth;
    element.classList.add("data-refresh-pulse");
    element.addEventListener(
      "animationend",
      () => element.classList.remove("data-refresh-pulse"),
      { once: true },
    );
  }
}

async function holdFeedbackFor(startedAt: number, minimumMs: number) {
  const remaining = minimumMs - (performance.now() - startedAt);
  if (remaining > 0) {
    await new Promise((resolve) => window.setTimeout(resolve, remaining));
  }
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

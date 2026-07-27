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

type GithubConnection = {
  provider: string;
  adapter: string;
  account: string;
  credential_profile: string;
  status: string;
};

type GithubWriteAuthorization = {
  connection: ProjectConnection;
  account: string;
  permission: string;
};

type GithubRepository = {
  name_with_owner: string;
  url: string;
  default_branch: string | null;
  is_private: boolean;
};

type OpenAiConnection = {
  provider: string;
  adapter: string;
  auth_method: string;
  credential_profile: string;
  status: string;
};

type Scene = "bridge" | "plan" | "engineering" | "crew";
type SetupStep = "github" | "project" | "openai" | "staff" | "review";
type FeedbackState = "idle" | "connecting" | "connected" | "preview" | "error";

const SETUP_COMPLETE_KEY = "ncc-1701-h:commissioning-complete:v2";
const STAFFING_COMPLETE_KEY = "ncc-1701-h:staffing-brief-complete:v2";
const setupSteps: SetupStep[] = ["github", "project", "openai", "staff", "review"];
const isNative = "__TAURI_INTERNALS__" in window;

let githubConnection: GithubConnection | null = null;
let githubRepositories: GithubRepository[] = [];
let projectConnection: ProjectConnection | null = null;
let openAiConnection: OpenAiConnection | null = null;
let staffingSubmitted = readStoredFlag(STAFFING_COMPLETE_KEY);
let onboardingComplete = readStoredFlag(SETUP_COMPLETE_KEY);
let currentSetupStep: SetupStep = "github";
let furthestSetupStep = 0;
const previewedSteps = new Set<SetupStep>();

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

const commissioning = document.querySelector<HTMLElement>("#commissioning");
const commandForm = document.querySelector<HTMLFormElement>("#command-form");
const commandInput = document.querySelector<HTMLInputElement>("#command-input");
const commandTranscript = document.querySelector<HTMLElement>("#command-transcript");

const githubButton = document.querySelector<HTMLButtonElement>("#setup-github-button");
const githubResult = document.querySelector<HTMLElement>("#setup-github-result");
const githubTitle = document.querySelector<HTMLElement>("#setup-github-title");
const githubDetail = document.querySelector<HTMLElement>("#setup-github-detail");
const githubAccountLabel = document.querySelector<HTMLElement>("#setup-github-account");
const githubPreviewNext = document.querySelector<HTMLButtonElement>("#setup-github-preview-next");

const projectForm = document.querySelector<HTMLFormElement>("#setup-project-form");
const projectAdapterInput = document.querySelector<HTMLInputElement>("#setup-project-adapter");
const projectPicker = document.querySelector<HTMLSelectElement>("#setup-project-picker");
const projectNameInput = document.querySelector<HTMLInputElement>("#setup-project-name");
const projectRepositoryInput = document.querySelector<HTMLInputElement>("#setup-project-repository");
const projectWorkspaceInput = document.querySelector<HTMLInputElement>("#setup-project-workspace");
const projectBranchInput = document.querySelector<HTMLInputElement>("#setup-project-branch");
const projectButton = document.querySelector<HTMLButtonElement>("#setup-project-button");
const projectResult = document.querySelector<HTMLElement>("#setup-project-result");
const projectTitle = document.querySelector<HTMLElement>("#setup-project-title");
const projectDetail = document.querySelector<HTMLElement>("#setup-project-detail");
const projectState = document.querySelector<HTMLElement>("#setup-project-state");
const projectPreviewNext = document.querySelector<HTMLButtonElement>("#setup-project-preview-next");
const bridgeEnableWrites = document.querySelector<HTMLButtonElement>("#bridge-enable-writes");

const openAiButton = document.querySelector<HTMLButtonElement>("#setup-openai-button");
const openAiResult = document.querySelector<HTMLElement>("#setup-openai-result");
const openAiTitle = document.querySelector<HTMLElement>("#setup-openai-title");
const openAiDetail = document.querySelector<HTMLElement>("#setup-openai-detail");
const openAiModelLabel = document.querySelector<HTMLElement>("#setup-openai-model");
const openAiPreviewNext = document.querySelector<HTMLButtonElement>("#setup-openai-preview-next");

const staffingForm = document.querySelector<HTMLFormElement>("#setup-staffing-form");
const staffingPrompt = document.querySelector<HTMLTextAreaElement>("#setup-staffing-prompt");
const staffingButton = document.querySelector<HTMLButtonElement>("#setup-staffing-button");
const staffingPreviewNext = document.querySelector<HTMLButtonElement>("#setup-staff-preview-next");
const enterBridgeButton = document.querySelector<HTMLButtonElement>("#setup-enter-bridge");

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

function readStoredFlag(key: string) {
  try {
    return window.localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

function storeFlag(key: string, value: boolean) {
  try {
    if (value) {
      window.localStorage.setItem(key, "true");
    } else {
      window.localStorage.removeItem(key);
    }
  } catch {
    // The native state remains authoritative if storage is unavailable.
  }
}

function setText(selector: string, text: string) {
  const element = document.querySelector(selector);
  if (element) element.textContent = text;
}

function setScene(name: Scene) {
  if (!onboardingComplete && name !== "crew") return;

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
          ? "Review connections, briefing, and model assignments."
          : "Independent models. One accountable command structure.",
  );
  setText(
    "#ship-location",
    `AGENT COMMAND HARNESS / ${isPlanMode ? "CONFERENCE" : isEngineering ? "ENGINEERING" : isCrew ? "CREW" : "BRIDGE"}`,
  );
  setText("#table-mode", isEngineering ? "ENGINEERING ROOM" : "PLAN MODE");
  setText("#table-title", isEngineering ? "BUG RESOLUTION" : "PLANNING SESSION");
  setText(
    "#table-subtitle",
    isEngineering ? "REPRODUCE / ISOLATE / PATCH / VERIFY" : "6 INDEPENDENT PERSPECTIVES",
  );

  if (commandInput) {
    commandInput.placeholder = isPlanMode
      ? "Give the planning team a problem to work through..."
      : isEngineering
        ? "Describe the bug, symptoms, and expected behaviour..."
        : isCrew
          ? "Crew configuration does not dispatch commands."
          : "Give the senior staff an objective...";
  }

  document.querySelectorAll<HTMLButtonElement>(".nav[data-scene]").forEach((button) => {
    button.classList.toggle("active", button.dataset.scene === name);
  });

  if (isCrew) showSetupStep("review");
}

function showSetupStep(step: SetupStep) {
  currentSetupStep = step;
  const stepIndex = setupSteps.indexOf(step);
  furthestSetupStep = Math.max(furthestSetupStep, stepIndex);
  if (commissioning) commissioning.dataset.step = step;

  document.querySelectorAll<HTMLElement>("[data-step-panel]").forEach((panel) => {
    const isActive = panel.dataset.stepPanel === step;
    panel.classList.toggle("is-active", isActive);
    panel.setAttribute("aria-hidden", String(!isActive));
  });

  const completed = completedSetupSteps();
  document.querySelectorAll<HTMLButtonElement>("[data-setup-target]").forEach((button) => {
    const target = button.dataset.setupTarget as SetupStep;
    const targetIndex = setupSteps.indexOf(target);
    const state = target === step
      ? "current"
      : completed.has(target)
        ? "complete"
        : previewedSteps.has(target)
          ? "preview"
          : "pending";
    button.dataset.state = state;
    button.disabled = !onboardingComplete && targetIndex > furthestSetupStep;
    if (target === step) button.setAttribute("aria-current", "step");
    else button.removeAttribute("aria-current");
  });

  renderReviewSummary();
  const activePanel = document.querySelector<HTMLElement>(`[data-step-panel="${step}"]`);
  activePanel?.querySelector<HTMLElement>("button, input, textarea")?.focus({ preventScroll: true });
}

function completedSetupSteps() {
  const complete = new Set<SetupStep>();
  if (githubConnection) complete.add("github");
  if (isNative && projectConnection) complete.add("project");
  if (openAiConnection) complete.add("openai");
  if (staffingSubmitted) complete.add("staff");
  if (onboardingComplete) complete.add("review");
  return complete;
}

function setOnboardingGate(isGated: boolean) {
  document.body.dataset.onboarding = isGated ? "active" : "complete";
  if (isGated) document.body.dataset.scene = "crew";
  if (enterBridgeButton) {
    enterBridgeButton.textContent = onboardingComplete
      ? "RETURN TO BRIDGE"
      : isNative
        ? "COMMISSION SHIP / ENTER BRIDGE"
        : "ENTER BRIDGE PREVIEW";
  }
}

function advanceFromPreview(step: SetupStep, next: SetupStep) {
  previewedSteps.add(step);
  furthestSetupStep = Math.max(furthestSetupStep, setupSteps.indexOf(next));
  showSetupStep(next);
}

document.querySelectorAll<HTMLButtonElement>("button[data-scene]").forEach((button) => {
  button.addEventListener("click", () => setScene(button.dataset.scene as Scene));
});

document.querySelectorAll<HTMLButtonElement>("[data-setup-target]").forEach((button) => {
  button.addEventListener("click", () => showSetupStep(button.dataset.setupTarget as SetupStep));
});

document.querySelectorAll<HTMLButtonElement>("[data-setup-back]").forEach((button) => {
  button.addEventListener("click", () => showSetupStep(button.dataset.setupBack as SetupStep));
});

document.querySelectorAll<HTMLButtonElement>("[data-setup-next]").forEach((button) => {
  button.addEventListener("click", () => {
    const next = button.dataset.setupNext as SetupStep;
    advanceFromPreview(currentSetupStep, next);
  });
});

commandForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const message = commandInput?.value.trim() ?? "";
  if (!message || !commandInput || !commandTranscript) return;

  commandInput.disabled = true;
  const submit = commandForm.querySelector<HTMLButtonElement>("button");
  if (submit) submit.disabled = true;

  try {
    if (isNative) {
      await invoke<SavedCommand>("submit_captain_message", {
        messageId: crypto.randomUUID(),
        text: message,
      });
      commandTranscript.innerHTML = `<b>PICARD</b><span>${escapeHtml(message)}</span>`;
      setText("#system-status", "COMMAND SAVED / AWAITING FIRST OFFICER");
    } else {
      commandTranscript.innerHTML =
        `<b>PREVIEW</b><span>${escapeHtml(message)} / Native Warp Core required to dispatch.</span>`;
    }
    commandInput.value = "";
  } catch (error) {
    setText("#system-status", `COMMAND REJECTED / ${String(error)}`);
  } finally {
    commandInput.disabled = false;
    if (submit) submit.disabled = false;
    commandInput.focus();
  }
});

githubButton?.addEventListener("click", async () => {
  githubButton.disabled = true;
  githubButton.textContent = isNative ? "OPENING SECURE SIGN-IN..." : "CHECKING NATIVE HANDOFF...";
  setGithubFeedback(
    "connecting",
    isNative ? "CONTACTING GITHUB" : "WEB PREVIEW",
    isNative ? "COMPLETE SIGN-IN IN YOUR BROWSER" : "AUTHENTICATION IS DISABLED IN PREVIEW",
  );
  const startedAt = performance.now();

  if (!isNative) {
    await holdFeedbackFor(startedAt, 550);
    setGithubFeedback("preview", "NATIVE SIGN-IN REQUIRED", "NO ACCOUNT WAS CONNECTED");
    githubButton.textContent = "SIGN IN WITH GITHUB";
    githubButton.disabled = false;
    if (githubPreviewNext) githubPreviewNext.hidden = false;
    setText("#system-status", "WEB PREVIEW / GITHUB SIGN-IN NOT ATTEMPTED");
    return;
  }

  try {
    githubConnection = await invoke<GithubConnection>("authorize_github_account");
    await holdFeedbackFor(startedAt, 550);
    renderGithubConnection(githubConnection);
    await loadGithubRepositories();
    furthestSetupStep = Math.max(furthestSetupStep, 1);
    setText("#system-status", `GITHUB CONNECTED / ${githubConnection.account.toUpperCase()}`);
    showSetupStep("project");
  } catch (error) {
    setGithubFeedback("error", "GITHUB SIGN-IN FAILED", String(error));
    githubButton.textContent = "RETRY GITHUB SIGN-IN";
    setText("#system-status", `GITHUB SIGN-IN FAILED / ${String(error)}`);
  } finally {
    githubButton.disabled = false;
  }
});

projectRepositoryInput?.addEventListener("input", () => {
  if (!projectNameInput || projectNameInput.value.trim()) return;
  const repository = projectRepositoryInput.value.trim().replace(/\.git$/, "");
  const name = repository.split("/").filter(Boolean).at(-1);
  if (name) projectNameInput.value = name;
});

projectPicker?.addEventListener("change", () => {
  const selected = githubRepositories.find((repository) => repository.url === projectPicker.value);
  if (!selected) return;
  if (projectRepositoryInput) projectRepositoryInput.value = selected.url;
  if (projectNameInput) {
    projectNameInput.value = selected.name_with_owner.split("/").at(-1) ?? selected.name_with_owner;
  }
  if (projectBranchInput) projectBranchInput.value = selected.default_branch || "main";
  setProjectFeedback(
    "idle",
    selected.name_with_owner.toUpperCase(),
    `${selected.is_private ? "PRIVATE" : "PUBLIC"} / ${selected.default_branch || "main"}`,
  );
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
  if (projectButton) {
    projectButton.disabled = true;
    projectButton.textContent = "CONTACTING PROJECT ADAPTER...";
  }
  setProjectFeedback("connecting", "CONTACTING GITHUB", "CHECKING PROJECT TARGET");
  const startedAt = performance.now();

  try {
    if (isNative) {
      projectConnection = await invoke<ProjectConnection>("connect_project", { connection });
      await holdFeedbackFor(startedAt, 500);
      renderProjectConnection(projectConnection);
      setProjectFeedback(
        "connected",
        "PROJECT CONNECTED / READ ONLY",
        `${projectConnection.display_name} / ${projectConnection.default_branch}`,
      );
      furthestSetupStep = Math.max(furthestSetupStep, 2);
      setText("#system-status", "PROJECT SAVED THROUGH WARP CORE / READ ONLY");
      showSetupStep("openai");
    } else {
      projectConnection = connection;
      await holdFeedbackFor(startedAt, 500);
      renderProjectConnection(connection);
      setProjectFeedback("preview", "PREVIEW CONFIGURATION ONLY", "NOT SAVED / NO REPOSITORY ACCESSED");
      if (projectPreviewNext) projectPreviewNext.hidden = false;
      setText("#system-status", "WEB PREVIEW / PROJECT CONNECTION NOT ATTEMPTED");
    }
  } catch (error) {
    setProjectFeedback("error", "PROJECT CONNECTION FAILED", String(error));
    if (projectButton) projectButton.textContent = "RETRY PROJECT CONNECTION";
    setText("#system-status", `PROJECT REJECTED / ${String(error)}`);
  } finally {
    if (projectButton) {
      projectButton.disabled = false;
      if (projectButton.textContent === "CONTACTING PROJECT ADAPTER...") {
        projectButton.textContent = isNative ? "UPDATE PROJECT" : "CONNECT THIS PROJECT";
      }
    }
  }
});

bridgeEnableWrites?.addEventListener("click", async () => {
  if (!isNative) {
    setText("#system-status", "WEB PREVIEW / OPEN THE DESKTOP APP TO ENABLE GITHUB CHANGES");
    pulseRefresh(bridgeEnableWrites);
    return;
  }

  bridgeEnableWrites.disabled = true;
  bridgeEnableWrites.textContent = "CHECKING GITHUB PERMISSION...";
  try {
    const authorization = await invoke<GithubWriteAuthorization>("authorize_github_writes");
    projectConnection = authorization.connection;
    renderProjectConnection(projectConnection);
    setText(
      "#system-status",
      `GITHUB CHANGES ENABLED / ${authorization.account.toUpperCase()} / ${authorization.permission}`,
    );
  } catch (error) {
    bridgeEnableWrites.hidden = false;
    bridgeEnableWrites.textContent = "RETRY ENABLE CHANGES";
    setText("#system-status", `GITHUB AUTHORIZATION FAILED / ${String(error)}`);
    pulseRefresh(bridgeEnableWrites);
  } finally {
    bridgeEnableWrites.disabled = false;
  }
});

openAiButton?.addEventListener("click", async () => {
  openAiButton.disabled = true;
  openAiButton.textContent = isNative ? "OPENING CHATGPT SIGN-IN..." : "CHECKING NATIVE HANDOFF...";
  setOpenAiFeedback(
    "connecting",
    isNative ? "CHECKING CODEX SESSION" : "WEB PREVIEW",
    isNative ? "A SIGNED-IN SESSION MAY BE REUSED" : "AUTHENTICATION IS DISABLED IN PREVIEW",
  );
  const startedAt = performance.now();

  if (!isNative) {
    await holdFeedbackFor(startedAt, 550);
    setOpenAiFeedback("preview", "NATIVE SIGN-IN REQUIRED", "NO OPENAI ACCOUNT WAS CONNECTED");
    openAiButton.textContent = "CONTINUE WITH CHATGPT";
    openAiButton.disabled = false;
    if (openAiPreviewNext) openAiPreviewNext.hidden = false;
    setText("#system-status", "WEB PREVIEW / OPENAI SIGN-IN NOT ATTEMPTED");
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
    await holdFeedbackFor(startedAt, 550);
    renderOpenAiConnection(openAiConnection);
    renderCrew(activeCrew);
    furthestSetupStep = Math.max(furthestSetupStep, 3);
    setText("#system-status", "OPENAI CONNECTED / PICARD COMMAND MODEL READY");
    showSetupStep("staff");
  } catch (error) {
    setOpenAiFeedback("error", "OPENAI SIGN-IN FAILED", String(error));
    openAiButton.textContent = "RETRY CHATGPT SIGN-IN";
    setText("#system-status", `OPENAI SIGN-IN FAILED / ${String(error)}`);
  } finally {
    openAiButton.disabled = false;
  }
});

staffingForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = staffingPrompt?.value.trim() ?? "";
  if (!prompt || !staffingPrompt) return;

  if (isNative && !openAiConnection) {
    setText("#system-status", "CONNECT OPENAI BEFORE BRIEFING PICARD");
    showSetupStep("openai");
    return;
  }

  if (staffingButton) {
    staffingButton.disabled = true;
    staffingButton.textContent = isNative ? "SENDING BRIEF TO PICARD..." : "PREPARING PREVIEW...";
  }

  try {
    if (isNative) {
      await invoke<SavedCommand>("submit_staffing_brief", {
        briefId: crypto.randomUUID(),
        prompt,
      });
      staffingSubmitted = true;
      storeFlag(STAFFING_COMPLETE_KEY, true);
      furthestSetupStep = Math.max(furthestSetupStep, 4);
      setText("#system-status", "STAFFING BRIEF SAVED / CREW READY FOR REVIEW");
      showSetupStep("review");
    } else {
      await holdFeedbackFor(performance.now(), 450);
      pulseRefresh(document.querySelector<HTMLElement>(".picard-brief"));
      if (staffingPreviewNext) staffingPreviewNext.hidden = false;
      setText("#system-status", "WEB PREVIEW / STAFFING BRIEF NOT DISPATCHED");
    }
  } catch (error) {
    setText("#system-status", `STAFFING BRIEF REJECTED / ${String(error)}`);
  } finally {
    if (staffingButton) {
      staffingButton.disabled = false;
      staffingButton.textContent = "ASK PICARD TO STAFF THE SHIP";
    }
  }
});

enterBridgeButton?.addEventListener("click", () => {
  onboardingComplete = true;
  staffingSubmitted = true;
  storeFlag(STAFFING_COMPLETE_KEY, true);
  storeFlag(SETUP_COMPLETE_KEY, true);
  setOnboardingGate(false);
  setScene("bridge");
  setText(
    "#system-status",
    isNative
      ? "SHIP COMMISSIONED / BRIDGE ONLINE"
      : "WEB PREVIEW / BRIDGE LAYOUT UNLOCKED",
  );
});

async function connectWarpCore() {
  renderCrew(activeCrew);

  if (!isNative) {
    document.body.dataset.runtime = "preview";
    setText("#walkthrough-mode", "WEB PREVIEW / NO AUTHENTICATION");
    setText("#system-status", "WEB PREVIEW / NO ACCOUNTS OR PROJECTS ARE ACCESSED");
    setGithubFeedback(
      "preview",
      "DESKTOP APP REQUIRED",
      "OPEN NCC-1701-H TO SIGN IN WITH GITHUB",
    );
    if (githubButton) {
      githubButton.textContent = "GITHUB SIGN-IN REQUIRES DESKTOP";
      githubButton.disabled = true;
    }
    if (githubPreviewNext) githubPreviewNext.hidden = false;
    setOpenAiFeedback(
      "preview",
      "DESKTOP APP REQUIRED",
      "OPEN NCC-1701-H TO SIGN IN WITH CHATGPT",
    );
    if (openAiButton) {
      openAiButton.textContent = "OPENAI SIGN-IN REQUIRES DESKTOP";
      openAiButton.disabled = true;
    }
    if (openAiPreviewNext) openAiPreviewNext.hidden = false;
    if (onboardingComplete) {
      setupSteps.forEach((step) => previewedSteps.add(step));
      setOnboardingGate(false);
      setScene("bridge");
    } else {
      setOnboardingGate(true);
      showSetupStep("github");
    }
    return;
  }

  try {
    const [crew, status, github, project, openAi] = await Promise.all([
      invoke<CrewManifest>("get_crew_manifest"),
      invoke<WarpCoreStatus>("get_warp_core_status"),
      invoke<GithubConnection | null>("get_github_connection"),
      invoke<ProjectConnection | null>("get_project_connection"),
      invoke<OpenAiConnection | null>("get_openai_connection"),
    ]);
    activeCrew = crew;
    githubConnection = github;
    projectConnection = project;
    openAiConnection = openAi;

    setText(
      "#system-status",
      `WARP CORE ONLINE / ${crew.leaders.length} OFFICERS / ${status.queued + status.retry} TO BASE`,
    );
    setText("#metric-core", "ONLINE");
    setText("#metric-officers", String(crew.leaders.length).padStart(2, "0"));
    setText("#metric-queue", String(status.queued + status.retry).padStart(2, "0"));
    renderCrew(crew);
    if (github) {
      renderGithubConnection(github);
      await loadGithubRepositories();
    }
    if (project) renderProjectConnection(project);
    if (openAi) renderOpenAiConnection(openAi);

    const firstIncomplete = getFirstIncompleteStep();
    if (onboardingComplete && firstIncomplete === "review") {
      setOnboardingGate(false);
      setScene("bridge");
      return;
    }

    if (onboardingComplete && firstIncomplete !== "review") {
      onboardingComplete = false;
      storeFlag(SETUP_COMPLETE_KEY, false);
    }
    setOnboardingGate(true);
    furthestSetupStep = setupSteps.indexOf(firstIncomplete);
    showSetupStep(firstIncomplete);
  } catch (error) {
    onboardingComplete = false;
    setOnboardingGate(true);
    showSetupStep("github");
    setText("#system-status", `WARP CORE OFFLINE / ${String(error)}`);
  }
}

function getFirstIncompleteStep(): SetupStep {
  if (!githubConnection) return "github";
  if (!projectConnection) return "project";
  if (!openAiConnection) return "openai";
  if (!staffingSubmitted) return "staff";
  return "review";
}

function renderCrew(crew: CrewManifest) {
  renderCommandModel(crew);
  renderModelAssignments(crew);
  renderModelSettings(crew);
}

function renderCommandModel(crew: CrewManifest) {
  if (!openAiConnection || !crew.command_model) {
    if (openAiModelLabel) openAiModelLabel.textContent = "MODEL / AFTER SIGN-IN";
    return;
  }
  if (openAiModelLabel) {
    openAiModelLabel.textContent =
      crew.command_model.model === "codex-default"
        ? "MODEL / AUTO"
        : `MODEL / ${crew.command_model.model.toUpperCase()}`;
  }
}

function renderGithubConnection(connection: GithubConnection) {
  setGithubFeedback(
    "connected",
    `SIGNED IN AS ${connection.account.toUpperCase()}`,
    `${connection.adapter.toUpperCase()} / ${connection.status.toUpperCase()}`,
  );
  if (githubAccountLabel) githubAccountLabel.textContent = `ACCOUNT / ${connection.account.toUpperCase()}`;
  if (githubButton) githubButton.textContent = "GITHUB CONNECTED";
  renderReviewSummary();
}

async function loadGithubRepositories() {
  if (!projectPicker) return;

  projectPicker.disabled = true;
  projectPicker.replaceChildren(repositoryOption("", "LOADING YOUR GITHUB REPOSITORIES..."));
  try {
    githubRepositories = await invoke<GithubRepository[]>("list_github_repositories");
    const prompt = repositoryOption(
      "",
      githubRepositories.length > 0
        ? `CHOOSE A REPOSITORY / ${githubRepositories.length} AVAILABLE`
        : "NO REPOSITORIES RETURNED / USE MANUAL URL",
    );
    projectPicker.replaceChildren(
      prompt,
      ...githubRepositories.map((repository) =>
        repositoryOption(
          repository.url,
          `${repository.name_with_owner}${repository.is_private ? " / PRIVATE" : ""}`,
        ),
      ),
    );
    const connectedRepository = projectConnection?.repository;
    if (
      connectedRepository &&
      githubRepositories.some((repository) => repository.url === connectedRepository)
    ) {
      projectPicker.value = connectedRepository;
    }
  } catch (error) {
    githubRepositories = [];
    projectPicker.replaceChildren(repositoryOption("", "REPOSITORY LIST UNAVAILABLE / USE MANUAL URL"));
    setText("#system-status", `GITHUB REPOSITORY LIST UNAVAILABLE / ${String(error)}`);
  } finally {
    projectPicker.disabled = false;
  }
}

function repositoryOption(value: string, label: string) {
  const option = document.createElement("option");
  option.value = value;
  option.textContent = label;
  return option;
}

function renderOpenAiConnection(connection: OpenAiConnection) {
  const method = connection.auth_method === "chatgpt" ? "CHATGPT" : connection.auth_method.toUpperCase();
  setOpenAiFeedback("connected", "OPENAI CONNECTED", `${method} / CODEX SESSION`);
  if (openAiButton) openAiButton.textContent = "OPENAI CONNECTED";
  renderCommandModel(activeCrew);
  renderReviewSummary();
}

function renderProjectConnection(connection: ProjectConnection) {
  setText("#project-adapter", connection.adapter.toUpperCase());
  setText("#project-name", connection.display_name.toUpperCase());
  setText(
    "#project-summary",
    `${connection.repository} / ${connection.default_branch} / ${connection.access === "read_write" ? "WRITE ENABLED" : "READ ONLY"}`,
  );
  if (projectAdapterInput) projectAdapterInput.value = connection.adapter;
  if (projectNameInput) projectNameInput.value = connection.display_name;
  if (projectRepositoryInput) projectRepositoryInput.value = connection.repository;
  if (projectWorkspaceInput) projectWorkspaceInput.value = connection.workspace_path ?? "";
  if (projectBranchInput) projectBranchInput.value = connection.default_branch;
  if (projectState) projectState.textContent = isNative ? "WARP CORE SAVED" : "PREVIEW ONLY";
  if (projectButton) projectButton.textContent = isNative ? "UPDATE PROJECT" : "CONNECT THIS PROJECT";
  if (bridgeEnableWrites) {
    bridgeEnableWrites.hidden =
      connection.adapter !== "github" || connection.access !== "read_only";
    bridgeEnableWrites.textContent = "ENABLE GITHUB CHANGES";
  }
  pulseRefresh(
    document.querySelector<HTMLElement>("#project-name"),
    document.querySelector<HTMLElement>("#project-adapter"),
  );
  renderReviewSummary();
}

function renderReviewSummary() {
  setText(
    "#review-github",
    isNative ? githubConnection?.account.toUpperCase() ?? "NOT CONNECTED" : "PREVIEW ONLY",
  );
  setText(
    "#review-project",
    isNative
      ? projectConnection?.display_name.toUpperCase() ?? "NOT SELECTED"
      : projectConnection?.display_name.toUpperCase() ?? "PREVIEW ONLY",
  );
  setText("#review-openai", isNative ? (openAiConnection ? "OPENAI" : "NOT CONNECTED") : "PREVIEW ONLY");
}

function renderModelAssignments(crew: CrewManifest) {
  const list = document.querySelector<HTMLElement>("#crew-models");
  if (!list) return;
  list.replaceChildren(
    ...crew.leaders.slice(0, 6).map((crewLeader) => {
      const row = document.createElement("div");
      const dot = document.createElement("i");
      dot.className = `dot ${departmentDotClass(crewLeader.department)}`;
      const name = document.createElement("strong");
      name.textContent = crewLeader.display_name.split(" ").at(-1) ?? crewLeader.display_name;
      const model = document.createElement("span");
      model.textContent = crewLeader.model.model;
      row.append(dot, name, model);
      return row;
    }),
  );
}

function renderModelSettings(crew: CrewManifest) {
  const settings = document.querySelector<HTMLElement>("#setup-model-settings");
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
        leaders: activeCrew.leaders.map((entry) =>
          entry.id === crewLeader.id ? { ...entry, model: assignment } : entry,
        ),
      };
    }
    renderCrew(activeCrew);
    setText(
      "#system-status",
      isNative
        ? `${crewLeader.display_name.toUpperCase()} / MODEL ASSIGNMENT SAVED`
        : `${crewLeader.display_name.toUpperCase()} / PREVIEW MODEL UPDATED / NOT SAVED`,
    );
  } catch (error) {
    setText("#system-status", `MODEL ASSIGNMENT REJECTED / ${String(error)}`);
    if (submit) submit.disabled = false;
  }
}

function setGithubFeedback(state: FeedbackState, title: string, detail: string) {
  setConnectionFeedback(githubResult, githubTitle, githubDetail, state, title, detail);
}

function setProjectFeedback(state: FeedbackState, title: string, detail: string) {
  setConnectionFeedback(projectResult, projectTitle, projectDetail, state, title, detail);
}

function setOpenAiFeedback(state: FeedbackState, title: string, detail: string) {
  setConnectionFeedback(openAiResult, openAiTitle, openAiDetail, state, title, detail);
}

function setConnectionFeedback(
  result: HTMLElement | null,
  titleElement: HTMLElement | null,
  detailElement: HTMLElement | null,
  state: FeedbackState,
  title: string,
  detail: string,
) {
  if (result) result.dataset.state = state;
  if (titleElement) titleElement.textContent = title;
  if (detailElement) detailElement.textContent = detail;
  if (state === "connected" || state === "preview" || state === "error") {
    pulseRefresh(result);
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

function escapeHtml(value: string) {
  const span = document.createElement("span");
  span.textContent = value;
  return span.innerHTML;
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

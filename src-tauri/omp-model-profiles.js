// Omp's RPC can switch the chat model, but not the settings used by advisor and subagents.
import { CHAT_MODEL_ROLE_IDS, isKindRole } from "@oh-my-pi/pi-coding-agent/config/model-roles";
import { lookup } from "@oh-my-pi/pi-coding-agent/config/registry";
import { AgentRegistry, MAIN_AGENT_ID } from "@oh-my-pi/pi-coding-agent/registry/agent-registry";
import { parseConfiguredThinkingLevel } from "@oh-my-pi/pi-tui/thinking";

function advisorThinkingLevel(state) {
	return state.thinkingLevel ?? (state.disableReasoning || !state.model?.reasoning ? "off" : "default");
}

function publishAdvisorModel(session, ui) {
	const state = session.getAdvisorAgent()?.state;
	const model = state?.model;
	ui.setStatus("bide-advisor-model", model?.name || model?.id || "");
	ui.setStatus("bide-advisor-thinking", model ? advisorThinkingLevel(state) : "");
}

export default function bideModelProfiles(pi) {
	let baselineRoles;
	let baselineThinking;
	let profileActive = false;
	let restoreRetrySettings;
	const publish = (_event, ctx) => {
		if (ctx.agent.kind !== "main") return;
		const session = AgentRegistry.global().get(MAIN_AGENT_ID)?.session;
		if (session) publishAdvisorModel(session, ctx.ui);
	};
	pi.on("session_start", publish);
	pi.on("agent_end", publish);

	pi.registerCommand("bide-model-profile", {
		description: "Apply Bide model and advisor settings in this process only",
		handler: async (args, ctx) => {
			const [ticket, encoded] = args.split(" ");
			if (!ticket || !encoded) return;
			let error;
			try {
				if (ctx.agent.kind !== "main") throw new Error("Model profiles apply only to the main session");
				const session = AgentRegistry.global().get(MAIN_AGENT_ID)?.session;
				const roles = lookup("modelRoles");
				const thinking = lookup("defaultThinkingLevel");
				if (!session || !roles || !thinking) throw new Error("Model profile settings unavailable");
				const selection = JSON.parse(Buffer.from(encoded, "base64").toString("utf8"));
				const managedRetry = Object.entries(selection.modelFallbacks ?? {});
				const retryOverrides = managedRetry.length ? [
					["retry.fallbackChains", Object.fromEntries(managedRetry.map(([role, model]) => [role, [model]]))],
					["retry.enabled", true],
					["retry.modelFallback", true],
					["retry.fallbackOn", "any"],
					["retry.rateLimitWalksChain", true],
				].map(([name, value]) => {
					const setting = lookup(name);
					if (!setting) throw new Error(`Omp does not support ${name}; update omp to use model fallbacks`);
					return [setting, value];
				}) : [];
				if (session.isStreaming) {
					throw new Error("Wait for the current response before changing models");
				}
				const settings = session.settings;
				const currentRoles = settings.getModelRoles();
				const runtimeRoles = Object.fromEntries(Object.entries(currentRoles)
					.filter(([role]) => settings.getModelRoleProvenance(role) === "runtime"));
				const baseRoles = baselineRoles ?? runtimeRoles;
				if (Object.hasOwn(selection, "advisorModel")) {
					const { advisorModel, advisorThinking, clearProfile } = selection;
					const advisorSelector = advisorThinking ? `${advisorModel}:${advisorThinking}` : advisorModel;
					const nextRoles = advisorModel ? { ...baseRoles, advisor: advisorSelector } : baseRoles;
					if (advisorModel) await session.modelRegistry.awaitBackgroundRefresh();
					const wasEnabled = session.isAdvisorEnabled();
					roles.override(settings, Object.keys(nextRoles).length ? nextRoles : undefined);
					session.setAdvisorEnabled(Boolean(advisorModel));
					const state = session.getAdvisorAgent()?.state;
					const model = state?.model;
					const matches = model && `${model.provider}/${model.id}` === advisorModel;
					const wrongLevel = matches && advisorThinking && !clearProfile &&
						advisorThinkingLevel(state) !== advisorThinking;
					if (advisorModel && (!matches || wrongLevel)) {
						if (!clearProfile) {
							roles.override(settings, Object.keys(runtimeRoles).length ? runtimeRoles : undefined);
							session.setAdvisorEnabled(wasEnabled);
							if (wrongLevel) {
								throw new Error(`Omp applied thinking level ${advisorThinkingLevel(state)} instead of ${advisorThinking}`);
							}
							throw new Error(model
								? `Omp resolved a different advisor model: ${model.provider}/${model.id}`
								: `Advisor model not available: ${advisorModel}`);
						}
						// An unavailable saved model must not trap the tab in a preset.
						roles.override(settings, Object.keys(baseRoles).length ? baseRoles : undefined);
						session.setAdvisorEnabled(false);
					}
					baselineRoles = matches ? baseRoles : undefined;
					if (clearProfile && profileActive) {
						thinking.override(settings, baselineThinking);
						profileActive = false;
						restoreRetrySettings?.();
						restoreRetrySettings = undefined;
					}
				} else {
					const profile = selection;
					const selector = profile.modelRoles.default;
					const slash = selector.indexOf("/");
					const provider = selector.slice(0, slash);
					const rawId = selector.slice(slash + 1);
					const colon = rawId.lastIndexOf(":");
					const suffix = colon < 0 ? undefined : parseConfiguredThinkingLevel(rawId.slice(colon + 1));
					await session.modelRegistry.awaitBackgroundRefresh();
					const models = session.getAvailableModels();
					const literal = models.find(m => m.provider === provider && m.id === rawId);
					const model = literal ?? (suffix === undefined ? undefined :
						models.find(m => m.provider === provider && m.id === rawId.slice(0, colon)));
					if (!model) throw new Error(`Model not available: ${selector}`);
					const configured = parseConfiguredThinkingLevel(profile.defaultThinkingLevel);
					if (profile.defaultThinkingLevel && configured === undefined) {
						throw new Error(`Unsupported thinking level: ${profile.defaultThinkingLevel}`);
					}
					const nextThinking = profileActive ? baselineThinking :
						thinking.provenance(settings) === "runtime" ? thinking.get(settings) : undefined;
					const nextRoles = { ...baseRoles };
					const chatRoles = new Set([...CHAT_MODEL_ROLE_IDS, ...Object.keys(currentRoles)]);
					for (const role of chatRoles) {
						if (!isKindRole(role) && !["advisor", "tiny", "memory"].includes(role)) nextRoles[role] = selector;
					}
					Object.assign(nextRoles, profile.modelRoles);
					await session.setModel(model, "default", { persist: false });
					restoreRetrySettings?.();
					restoreRetrySettings = undefined;
					if (retryOverrides.length) {
						const previous = retryOverrides.map(([setting]) => [
							setting, setting.provenance(settings) === "runtime" ? setting.get(settings) : undefined,
						]);
						for (const [setting, value] of retryOverrides) {
							// Keep the user's other chains and let omp resolve and retry the turn in place.
							setting.override(settings, typeof value === "object" ? { ...setting.get(settings), ...value } : value);
						}
						restoreRetrySettings = () => {
							for (const [setting, value] of previous) setting.override(settings, value);
						};
					}
					baselineRoles = baseRoles;
					baselineThinking = nextThinking;
					profileActive = true;
					// Omp allows session-level off, but excludes it from the default setting.
					thinking.override(settings, configured === "off" ? nextThinking : configured ?? nextThinking);
					roles.override(settings, nextRoles);
					session.setThinkingLevel((literal ? undefined : suffix) ?? configured ?? thinking.get(settings));
					session.setAdvisorEnabled(Boolean(profile.modelRoles.advisor));
				}
				publishAdvisorModel(session, ctx.ui);
			} catch (cause) {
				error = cause instanceof Error ? cause.message : String(cause);
			}
			ctx.ui.setStatus(`bide-model-profile:${ticket}`, JSON.stringify({ error }));
		},
	});
}

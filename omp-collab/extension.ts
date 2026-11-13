// Hosts omp Collab for bide's `omp --mode rpc-ui` sessions. omp's RPC mode never builds the
// CollabController the TUI uses, so this extension builds one around the main AgentSession.
// Bundled by omp-collab/build.ts; every other @oh-my-pi import binds to omp's own in-process modules.
import { CollabController } from "@oh-my-pi/pi-coding-agent/collab/controller";
import { lookup } from "@oh-my-pi/pi-coding-agent/config/registry";
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent/extensibility/extensions/types";
import type { InteractiveModeContext } from "@oh-my-pi/pi-coding-agent/modes/types";
import { AgentRegistry, MAIN_AGENT_ID } from "@oh-my-pi/pi-coding-agent/registry/agent-registry";

declare const OMP_COLLAB_BUILT_FOR: string;

function log(message: string): void {
	// stdout carries RPC frames; diagnostics go to stderr (bide's stderr tail).
	process.stderr.write(`[bide-collab] ${message}\n`);
}

export default function bideCollab(pi: ExtensionAPI): void {
	let controller: CollabController | undefined;

	pi.on("session_start", (_event, ctx) => {
		// Factories are rebound to every subagent session; only the top-level session hosts.
		if (ctx.agent.kind !== "main" || controller) return;
		const session = AgentRegistry.global().get(MAIN_AGENT_ID)?.session;
		if (!session) {
			log("main session not registered; collab disabled");
			return;
		}
		// Runtime-only (never written to config.yml): host a full-control room and follow session switches.
		lookup("collab.autoStart")?.override(session.settings, "control");
		// The members of InteractiveModeContext that CollabController/CollabHost read; the TUI-only ones are no-ops.
		const hostContext = {
			session,
			sessionManager: session.sessionManager,
			settings: session.settings,
			eventBus: pi.events,
			collabHost: undefined,
			collabGuest: undefined,
			showStatus: (message: string) => log(message),
			updatePendingMessagesDisplay: () => {},
			ui: { requestRender: () => {} },
			statusLine: {
				setCollabStatus: () => {},
				invalidate: () => {},
				getCachedContextBreakdown: () => session.getContextBreakdown() ?? { usedTokens: 0, contextWindow: 0 },
			},
		};
		// Unchecked: a structural subset of InteractiveModeContext, covering what collab/ reads in this omp version.
		controller = new CollabController(hostContext as unknown as InteractiveModeContext);
		controller.autoStart();
		// RPC startup has finished by the time extensions see session_start: let guests prompt.
		controller.startupComplete();
		log(`hosting collab (built for omp ${OMP_COLLAB_BUILT_FOR}, instance ${controller.instanceId})`);
	});

	pi.on("session_shutdown", async (_event, ctx) => {
		if (ctx.agent.kind !== "main") return;
		await controller?.shutdown("host exited");
	});
}

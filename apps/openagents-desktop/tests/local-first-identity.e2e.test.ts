import {describe,expect,test} from "bun:test"
import {validateBehaviorContractRegistry} from "@openagentsinc/behavior-contracts"
import {openAgentsDesktopUxContractRegistry} from "../src/contracts/ux-contracts"
import {settingsView,initialSettingsState} from "../src/renderer/settings"
describe("openagents_desktop.seam.identity.local_first_account_link.v1",()=>{test("registers the enforced Desktop local-first contract",()=>expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true));test("renders local usability and optional account linking",()=>{const view=JSON.stringify(settingsView({...initialSettingsState(),openAgentsSession:"signed_out"}));expect(view).toContain("Local device ready");expect(view).toContain("Link OpenAgents account");expect(view).toContain("never deletes local work")})})

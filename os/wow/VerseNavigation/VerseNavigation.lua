-- Firm reason: 6 (World of Warcraft's Lua-only addon SDK: public API sampling and visible pixel transport; controls and admission stay in Coax.)
-- SDK effect adapter only. This addon never moves, turns, targets, casts or invokes actions.
local frame = CreateFrame("Frame", "VerseNavigationSensor", UIParent)
frame:SetFrameStrata("TOOLTIP")
frame:SetScale(1 / UIParent:GetEffectiveScale())
frame:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 0, 0)
frame:SetSize(64, 8)
local cells = {}
for i = 1, 8 do
    local cell = frame:CreateTexture(nil, "OVERLAY")
    cell:SetPoint("TOPLEFT", frame, "TOPLEFT", (i - 1) * 8, 0)
    cell:SetSize(8, 8)
    cells[i] = cell
end
local serial = 0
local elapsed = 0
local function word(bytes, at, value)
    value = math.floor(value)
    bytes[at] = math.floor(value / 256) % 256
    bytes[at + 1] = value % 256
end
frame:SetScript("OnUpdate", function(_, dt)
    elapsed = elapsed + dt
    if elapsed < 0.1 then return end
    elapsed = 0
    serial = (serial + 1) % 65536
    local map = C_Map.GetBestMapForUnit("player")
    local position = map and C_Map.GetPlayerMapPosition(map, "player")
    local facing = GetPlayerFacing()
    local valid = position and facing and map and map > 0 and map < 65536
    local blocked = (GameMenuFrame and GameMenuFrame:IsShown()) or (SettingsPanel and SettingsPanel:IsShown()) or ChatEdit_GetActiveWindow()
    local flags = (valid and 1 or 0) + (not UnitIsDeadOrGhost("player") and 2 or 0) + (UnitAffectingCombat("player") and 4 or 0) + (blocked and 8 or 0)
    local b = {86, 78, 1, 0, 0, flags, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    word(b, 4, serial)
    if valid then
        word(b, 7, map)
        word(b, 10, position.x * 65535 + 0.5)
        word(b, 13, position.y * 65535 + 0.5)
        word(b, 16, facing / (2 * math.pi) * 65535 + 0.5)
    end
    local checksum = 0
    for i = 1, 23 do checksum = checksum + b[i] end
    b[24] = checksum % 256
    for i = 1, 8 do cells[i]:SetColorTexture(b[3*i-2] / 255, b[3*i-1] / 255, b[3*i] / 255, 1) end
end)

-- external-combat-v1: additional public SDK samples, with no action effects.
local combatFrame = CreateFrame("Frame", "VerseCombatSensor", UIParent)
combatFrame:SetFrameStrata("TOOLTIP")
combatFrame:SetScale(1 / UIParent:GetEffectiveScale())
combatFrame:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 0, -12)
combatFrame:SetSize(448, 8)
local combatCells = {}
for i = 1, 56 do
    local cell = combatFrame:CreateTexture(nil, "OVERLAY")
    cell:SetPoint("TOPLEFT", combatFrame, "TOPLEFT", (i - 1) * 8, 0)
    cell:SetSize(8, 8)
    combatCells[i] = cell
end
local targetEpoch, errorSerial, errorType = 0, 0, 0
local killSerial, killNpc, killSpawn, killTime = 0, 0, 0, 0
combatFrame:RegisterEvent("PLAYER_TARGET_CHANGED")
combatFrame:RegisterEvent("UI_ERROR_MESSAGE")
combatFrame:RegisterEvent("COMBAT_LOG_EVENT_UNFILTERED")
combatFrame:SetScript("OnEvent", function(_, event, value)
    if event == "PLAYER_TARGET_CHANGED" then targetEpoch = (targetEpoch + 1) % 65536 end
    if event == "UI_ERROR_MESSAGE" then errorSerial = (errorSerial + 1) % 65536; errorType = value or 0 end
    if event == "COMBAT_LOG_EVENT_UNFILTERED" then
        local _, kind, _, source, _, _, _, destination = CombatLogGetCurrentEventInfo()
        if kind == "PARTY_KILL" and source == UnitGUID("player") then
            local npc, spawn = (destination or ""):match("^[^-]+%-[^-]+%-[^-]+%-[^-]+%-[^-]+%-(%d+)%-([%x]+)$")
            killSerial = (killSerial + 1) % 65536
            killNpc = tonumber(npc) or 0; killSpawn = spawn and tonumber(spawn:sub(-8), 16) or 0; killTime = GetTime() * 1000
        end
    end
end)
local function put(b, at, value, width)
    value = math.max(0, math.min(256 ^ width - 1, math.floor(value or 0)))
    for j = width - 1, 0, -1 do b[at + j + 1] = value % 256; value = math.floor(value / 256) end
end
local function remaining(start, duration)
    return math.max(0, ((start or 0) + (duration or 0) - GetTime()) * 1000)
end
local function auraRemaining(unit, wanted, filter)
    for i = 1, 40 do
        local name, _, _, _, duration, expiration, _, _, _, spell = UnitAura(unit, i, filter)
        if not name then break end
        if spell == wanted then return duration == 0 and 65535 or math.max(0, (expiration - GetTime()) * 1000) end
    end
    return 0
end
local combatElapsed, combatSerial = 0, 0
local sensorError = combatFrame:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
sensorError:SetPoint("TOPLEFT", combatFrame, "BOTTOMLEFT", 0, -2)
local function combatUpdate(_, dt)
    combatElapsed = combatElapsed + dt
    if combatElapsed < 0.1 then return end
    combatElapsed = 0; combatSerial = (combatSerial + 1) % 65536
    local b = {}; for i = 1, 168 do b[i] = 0 end
    b[1], b[2], b[3] = 86, 67, 1
    put(b, 3, combatSerial, 2)
    local blocked = (GameMenuFrame and GameMenuFrame:IsShown()) or (SettingsPanel and SettingsPanel:IsShown()) or ChatEdit_GetActiveWindow()
    local cast, _, _, _, castEnd = UnitCastingInfo("player")
    b[6] = (UnitExists("player") and 1 or 0) + (not UnitIsDeadOrGhost("player") and 2 or 0) + (UnitAffectingCombat("player") and 4 or 0) + (blocked and 8 or 0) + (cast and 16 or 0) + ((GetUnitSpeed("player") or 0) > 0 and 32 or 0)
    local _, _, class = UnitClass("player")
    put(b, 6, UnitLevel("player"), 1); put(b, 7, class, 1)
    put(b, 8, UnitHealth("player"), 2); put(b, 10, UnitHealthMax("player"), 2)
    put(b, 12, UnitPower("player", 0), 2); put(b, 14, UnitPowerMax("player", 0), 2)
    put(b, 16, UnitXP("player"), 4); put(b, 20, UnitXPMax("player"), 4)
    local exists = UnitExists("target")
    local difficulty = exists and GetCreatureDifficultyColor(UnitLevel("target"))
    b[25] = (exists and 1 or 0) + (UnitCanAttack("player", "target") and 2 or 0) + (UnitIsDeadOrGhost("target") and 4 or 0) + (UnitPlayerControlled("target") and 8 or 0) + (UnitIsTapDenied("target") and 16 or 0) + (difficulty and difficulty == QuestDifficultyColors["trivial"] and 32 or 0) + (UnitClassification("target") == "normal" and 64 or 0) + (UnitAffectingCombat("target") and 128 or 0)
    put(b, 25, UnitLevel("target"), 1); put(b, 26, UnitHealth("target"), 2); put(b, 28, UnitHealthMax("target"), 2)
    put(b, 30, targetEpoch, 2)
    local guid = UnitGUID("target") or ""
    local npc, spawn = guid:match("^[^-]+%-[^-]+%-[^-]+%-[^-]+%-[^-]+%-(%d+)%-([%x]+)$")
    put(b, 32, tonumber(npc) or 0, 4); put(b, 36, spawn and tonumber(spawn:sub(-8), 16) or 0, 4)
    put(b, 40, castEnd and math.max(0, castEnd - GetTime() * 1000) or 0, 2)
    put(b, 42, errorSerial, 2); put(b, 44, errorType, 2)
    for slot = 1, 12 do
        local kind, id = GetActionInfo(slot)
        local usable, noPower = IsUsableAction(slot)
        local range = IsActionInRange(slot)
        local start, duration = GetActionCooldown(slot)
        local at = 48 + (slot - 1) * 7
        b[at + 1] = (kind and 1 or 0) + (usable and 2 or 0) + (noPower and 4 or 0) + (range ~= nil and 8 or 0) + ((range == true or range == 1) and 16 or 0) + (IsCurrentAction(slot) and 32 or 0) + (IsAttackAction(slot) and 64 or 0)
        put(b, at + 1, id, 3); put(b, at + 4, remaining(start, duration), 2); put(b, at + 6, GetActionCount(slot), 1)
    end
    put(b, 132, auraRemaining("player", 687, "HELPFUL"), 2)
    put(b, 134, auraRemaining("target", 172, "HARMFUL|PLAYER"), 2)
    put(b, 136, auraRemaining("target", 348, "HARMFUL|PLAYER"), 2)
    -- This client restricts measuring nameplate geometry. Leave optional plate
    -- coordinates unavailable rather than freezing the rest of the sample.
    if UnitIsUnit("targettarget", "player") then b[143] = b[143] + 2 end
    put(b, 144, killSerial, 2); put(b, 146, killNpc, 4); put(b, 150, killSpawn, 4); put(b, 154, killTime, 4)
    -- Transport CRC detects choosing a wrong UI-scale pitch across this wider strip.
    local crc = 0xffffffff
    for i = 1, 164 do
        crc = bit.bxor(crc, b[i])
        for _ = 1, 8 do
            if bit.band(crc, 1) ~= 0 then crc = bit.bxor(bit.rshift(crc, 1), 0xedb88320)
            else crc = bit.rshift(crc, 1) end
        end
    end
    put(b, 164, bit.bxor(crc, 0xffffffff) % 4294967296, 4)
    for i = 1, 56 do combatCells[i]:SetColorTexture(b[3*i-2] / 255, b[3*i-1] / 255, b[3*i] / 255, 1) end
    return true
end
combatFrame:SetScript("OnUpdate", function(self, dt)
    local ok, result = pcall(combatUpdate, self, dt)
    if not ok then sensorError:SetText("Verse combat sensor: " .. tostring(result))
    elseif result then sensorError:SetText("") end
end)

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

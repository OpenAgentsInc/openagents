import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { Textarea } from "./components/ui/textarea";
import { useAtom, useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import {
  DEFAULT_ENVIRONMENT,
  type NumericEnvironmentKey,
  type Tool,
} from "../../../packages/editor-services/protocol";
import { uiAtoms } from "./ui-state";
import { Icon } from "./icons";

export function Panel({
  tool,
  service,
  propsOnly = false,
}: {
  tool: Tool;
  service: EditorService;
  propsOnly?: boolean;
}) {
  return (
    <section
      className={`tool-panel tool-panel--${tool}`}
      aria-label={`${tool} tools`}
    >
      <div className="panel-title">
        <span>
          {
            {
              select: "Selection",
              assets: "Add to your world",
              build: "Build your town",
              command: "Describe a change",
              settings: "Light and atmosphere",
              shader: "Advanced material shader",
            }[tool]
          }
        </span>
        <Button
          variant="secondary"
          className="icon-button"
          aria-label="Close tool"
          onClick={() => service.registry.set(editorAtoms.tool, null)}
        >
          <Icon name="close" size={17} />
        </Button>
      </div>
      {tool === "assets" || tool === "build" ? (
        <AssetPanel service={service} buildingsOnly={tool === "build"} />
      ) : null}
      {tool === "command" ? (
        <CommandPanel service={service} propsOnly={propsOnly} />
      ) : null}
      {tool === "settings" ? <SettingsPanel service={service} /> : null}
      {tool === "select" ? (
        <SelectionPanel service={service} propsOnly={propsOnly} />
      ) : null}
      {tool === "shader" ? <ShaderPanel service={service} /> : null}
    </section>
  );
}
function AssetPanel({
  service,
  buildingsOnly,
}: {
  service: EditorService;
  buildingsOnly: boolean;
}) {
  const assets = useAtomValue(editorAtoms.assets);
  const status = useAtomValue(editorAtoms.status);
  const [search, setSearch] = useAtom(uiAtoms.assetSearch);
  const [choice, setChoice] = useAtom(uiAtoms.assetChoice);
  const [count, setCount] = useAtom(uiAtoms.count);
  const [placement, setPlacement] = useAtom(uiAtoms.placement);
  const filtered = assets.filter(
    (asset) =>
      (!buildingsOnly ||
        /house|building|cottage|manor|roof|wall|tower|archway/i.test(
          asset.name,
        ) ||
        /building|architecture/i.test(asset.category)) &&
      `${asset.name} ${asset.category}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const selected = assets.find((asset) => asset.id === choice) ?? filtered[0];
  return (
    <>
      {buildingsOnly ? (
        <div className="build-actions">
          <p className="muted">Add an editable room beside you.</p>
          <div className="button-pair">
            <Button
              variant="secondary"
              className="secondary-button"
              disabled={status.phase !== "ready"}
              onClick={() =>
                service.command({ kind: "room.add", side: placement, size: 6 })
              }
            >
              <Icon name="plus" size={16} />
              Add room
            </Button>
            <Button
              variant="secondary"
              className="secondary-button"
              disabled={status.phase !== "ready"}
              onClick={() => service.command({ kind: "wall.remove" })}
            >
              Remove aimed wall
            </Button>
          </div>
        </div>
      ) : null}
      <label className="search-field">
        <Icon name="search" size={17} />
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={
            buildingsOnly
              ? "Find a house or building…"
              : "Search houses, props, foliage…"
          }
          aria-label="Search assets"
        />
      </label>
      <div className="asset-grid" role="list" aria-label="Available assets">
        {filtered.slice(0, 80).map((asset) => (
          <Button
            variant="secondary"
            role="listitem"
            className={`asset-card ${selected?.id === asset.id ? "is-selected" : ""}`}
            key={asset.id}
            onClick={() => setChoice(asset.id)}
            aria-pressed={selected?.id === asset.id}
            title={asset.name}
          >
            <span className="asset-art">
              <Icon
                name={
                  /house|building|cottage|manor|tower/i.test(asset.name) ||
                  /building|architecture/i.test(asset.category)
                    ? "build"
                    : "assets"
                }
                size={31}
              />
            </span>
            <span className="asset-name">{asset.name}</span>
            <span className="asset-category">{asset.category}</span>
          </Button>
        ))}
      </div>
      {filtered.length === 0 ? (
        <p className="empty-state">
          {assets.length === 0
            ? "The asset library is loading with the world."
            : "No matching assets. Try a different name."}
        </p>
      ) : null}
      {filtered.length > 80 ? (
        <p className="muted">
          Showing 80 of {filtered.length}. Search to narrow the list.
        </p>
      ) : null}
      <div className="placement-row">
        <label>
          Count
          <Input
            aria-label="Asset count"
            type="number"
            min="1"
            max="8"
            value={count}
            onChange={(e) =>
              setCount(Math.max(1, Math.min(8, Number(e.target.value) || 1)))
            }
          />
        </label>
        <label>
          Place
          <select
            aria-label="Placement"
            value={placement}
            onChange={(e) => setPlacement(e.target.value as typeof placement)}
          >
            <option value="ahead">In front of me</option>
            <option value="left">To my left</option>
            <option value="right">To my right</option>
          </select>
        </label>
      </div>
      <Button
        variant="secondary"
        className="primary-button"
        disabled={!selected || status.phase !== "ready"}
        onClick={() =>
          selected &&
          service.command({
            kind: "asset.add",
            assetId: selected.id,
            count,
            placement,
          })
        }
      >
        <Icon name="plus" size={17} />
        {selected
          ? `Add ${count > 1 ? `${count} × ` : ""}${selected.name}`
          : "Add asset"}
      </Button>
      <p className="panel-footnote">Placed nearby. Undo brings you back.</p>
    </>
  );
}
function CommandPanel({
  service,
  propsOnly,
}: {
  service: EditorService;
  propsOnly?: boolean;
}) {
  const [text, setText] = useAtom(uiAtoms.commandText);
  const pending = useAtomValue(editorAtoms.pending);
  const status = useAtomValue(editorAtoms.status);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (text.trim())
          service.command({ kind: "text", text: text.trim().slice(0, 1200) });
      }}
    >
      <label className="sr-only" htmlFor="world-command">
        Describe a world change
      </label>
      <Textarea
        id="world-command"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            e.currentTarget.form?.requestSubmit();
          }
        }}
        placeholder={
          propsOnly
            ? "Add two cauldrons to my left"
            : "Add three barrels to my left"
        }
        maxLength={1200}
        rows={3}
      />
      <div className="suggestions">
        {(propsOnly
          ? [
              "Add a cauldron in front of me",
              "Add two wooden chests to my left",
              "Undo",
            ]
          : [
              "Add a cottage in front of me",
              "Add two barrels to my left",
              "Undo",
            ]
        ).map((example) => (
          <Button
            variant="secondary"
            type="button"
            key={example}
            onClick={() => setText(example)}
          >
            {example}
          </Button>
        ))}
      </div>
      <Button
        variant="secondary"
        className="primary-button"
        disabled={!text.trim() || status.phase !== "ready" || pending >= 16}
        type="submit"
      >
        Apply change
        <Icon name="arrow" size={18} />
      </Button>
      <p className="panel-footnote">Known actions update the world directly.</p>
    </form>
  );
}
function SelectionPanel({
  service,
  propsOnly,
}: {
  service: EditorService;
  propsOnly?: boolean;
}) {
  const status = useAtomValue(editorAtoms.status);
  const assets = useAtomValue(editorAtoms.assets);
  const selection = status.selection;
  if (!selection)
    return (
      <div className="empty-selection">
        <Icon name="select" size={34} />
        <p>Choose an object in the world.</p>
        <span className="muted">
          Click a placed object to inspect or remove it.
        </span>
      </div>
    );
  return (
    <>
      <div className="selected-object">
        {!propsOnly ? <span className="eyebrow">{selection.kind}</span> : null}
        <strong>
          {propsOnly
            ? (assets.find((asset) => asset.id === selection.kind)?.name ??
              "Selected prop")
            : selection.name}
        </strong>
      </div>
      <div className="coordinate-fields">
        {(["X", "Y", "Z"] as const).map((axis, index) => (
          <label key={`${selection.id}-${axis}-${selection.position[index]}`}>
            {axis}
            <Input
              aria-label={`${axis} position`}
              type="number"
              step="0.25"
              defaultValue={selection.position[index].toFixed(2)}
              onBlur={(event) => {
                const number = Number(event.target.value);
                if (!Number.isFinite(number)) return;
                const position: [number, number, number] = [
                  ...selection.position,
                ];
                position[index] = number;
                if (position[index] !== selection.position[index])
                  service.command({
                    kind: "entity.transform",
                    id: selection.id,
                    position,
                  });
              }}
            />
          </label>
        ))}
      </div>
      <label className="scale-field">
        Scale
        <Input
          aria-label="Object scale"
          type="number"
          min="0.25"
          max="4"
          step="0.25"
          key={`${selection.id}-${selection.scale}`}
          defaultValue={selection.scale}
          onBlur={(e) => {
            const scale = Number(e.target.value);
            if (
              Number.isFinite(scale) &&
              scale >= 0.25 &&
              scale <= 4 &&
              scale !== selection.scale
            )
              service.command({
                kind: "entity.transform",
                id: selection.id,
                scale,
              });
          }}
        />
      </label>
      <Button
        variant="secondary"
        className="danger-button"
        onClick={() =>
          service.command({ kind: "entity.remove", id: selection.id })
        }
      >
        <Icon name="trash" size={16} />
        Remove object
      </Button>
    </>
  );
}
const settingGroups: Record<
  "light" | "sky" | "surface",
  {
    key: NumericEnvironmentKey;
    label: string;
    min: number;
    max: number;
    step: number;
  }[]
> = {
  light: [
    { key: "exposure", label: "Exposure", min: 0.1, max: 4, step: 0.05 },
    { key: "sunIntensity", label: "Sunlight", min: 0, max: 100, step: 0.1 },
    { key: "sunAzimuth", label: "Sun direction", min: 0, max: 360, step: 1 },
    { key: "sunElevation", label: "Sun height", min: -20, max: 90, step: 1 },
    {
      key: "ambientIntensity",
      label: "Shade fill",
      min: 0,
      max: 8,
      step: 0.05,
    },
    { key: "moonIntensity", label: "Moonlight", min: 0, max: 100, step: 0.1 },
    { key: "bloomStrength", label: "Light glow", min: 0, max: 1, step: 0.01 },
  ],
  sky: [
    {
      key: "skyIntensity",
      label: "Sky strength",
      min: 0,
      max: 3,
      step: 0.05,
    },
    {
      key: "skyBackgroundIntensity",
      label: "Background brightness",
      min: 0,
      max: 4,
      step: 0.05,
    },
    { key: "cloudCoverage", label: "Cloud cover", min: 0, max: 1, step: 0.01 },
    { key: "cloudSpeed", label: "Cloud movement", min: 0, max: 2, step: 0.05 },
    {
      key: "fogDensity",
      label: "Distance haze",
      min: 0,
      max: 0.06,
      step: 0.001,
    },
  ],
  surface: [
    { key: "roughness", label: "Roughness", min: 0.1, max: 2, step: 0.05 },
    {
      key: "normalStrength",
      label: "Surface detail",
      min: 0,
      max: 2,
      step: 0.05,
    },
    { key: "saturation", label: "Color", min: 0, max: 2, step: 0.05 },
  ],
};
function SettingsPanel({ service }: { service: EditorService }) {
  const env = useAtomValue(editorAtoms.environment);
  const sourceSkyAvailable = useAtomValue(editorAtoms.sourceSkyAvailable);
  const sourceSkyLightIntensity = useAtomValue(
    editorAtoms.sourceSkyLightIntensity,
  );
  const [tab, setTab] = useAtom(uiAtoms.settingsTab);
  return (
    <>
      <div className="panel-tabs">
        {(["light", "sky", "surface"] as const).map((value) => (
          <Button
            variant="secondary"
            key={value}
            className={tab === value ? "active" : ""}
            aria-pressed={tab === value}
            onClick={() => setTab(value)}
          >
            {value === "surface"
              ? "Materials"
              : value[0].toUpperCase() + value.slice(1)}
          </Button>
        ))}
      </div>
      {tab === "sky" && sourceSkyAvailable ? (
        <div className="panel-tabs" role="group" aria-label="Sky source">
          {(["kit", "generated"] as const).map((source) => (
            <Button
              variant="secondary"
              key={source}
              className={env.skySource === source ? "active" : ""}
              aria-pressed={env.skySource === source}
              onClick={() => service.skySource(source)}
            >
              {source === "kit" ? "Kit sky" : "Generated sky"}
            </Button>
          ))}
        </div>
      ) : null}
      <div className="setting-list">
        {settingGroups[tab]
          .filter(
            (setting) =>
              (setting.key !== "skyBackgroundIntensity" ||
                env.skySource === "kit") &&
              (env.skySource === "generated" ||
                (setting.key !== "cloudCoverage" &&
                  setting.key !== "cloudSpeed")),
          )
          .map((setting) => (
            <label className="slider-label" key={setting.key}>
              <span>
                {setting.label}
                <span className="setting-value" aria-hidden="true">
                  {(env[setting.key] ?? 1).toFixed(
                    setting.step < 0.01 ? 3 : setting.step < 1 ? 2 : 0,
                  )}
                </span>
              </span>
              <Input
                aria-label={setting.label}
                type="range"
                min={setting.min}
                max={setting.max}
                step={setting.step}
                value={env[setting.key] ?? 1}
                onChange={(e) =>
                  service.environment({
                    ...env,
                    [setting.key]: Number(e.target.value),
                  })
                }
              />
            </label>
          ))}
      </div>
      <Button
        variant="secondary"
        className="quiet-button"
        onClick={() =>
          service.environment({
            ...DEFAULT_ENVIRONMENT,
            mode: env.mode,
            skySource: env.skySource,
            ambientIntensity:
              env.skySource === "kit"
                ? sourceSkyLightIntensity
                : DEFAULT_ENVIRONMENT.ambientIntensity,
          })
        }
      >
        Reset lighting and materials
      </Button>
      <p className="panel-footnote">
        Live settings. Reference views are starting compositions.
      </p>
    </>
  );
}

function ShaderPanel({ service }: { service: EditorService }) {
  const [source, setSource] = useAtom(editorAtoms.shaderSource);
  const working = useAtomValue(editorAtoms.activeShader);
  const status = useAtomValue(editorAtoms.status);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        service.shader(source);
      }}
    >
      <div className="shader-caption">
        <span>WGSL material</span>
        <span>
          {new TextEncoder().encode(source).byteLength.toLocaleString()} /
          65,536 bytes
        </span>
      </div>
      <Textarea
        className="shader-source"
        aria-label="WGSL material source"
        value={source}
        spellCheck={false}
        onChange={(event) => setSource(event.target.value)}
        maxLength={65536}
      />
      <div className="button-pair">
        <Button
          variant="secondary"
          type="button"
          className="secondary-button"
          onClick={() => setSource(working)}
          disabled={!working}
        >
          Restore working source
        </Button>
        <Button
          variant="secondary"
          className="primary-button"
          type="submit"
          disabled={status.phase !== "ready" || !source.trim()}
        >
          Apply shader
        </Button>
      </div>
      <p className="panel-footnote">
        Changes apply when compilation succeeds. Your last working shader keeps
        running on errors.
      </p>
    </form>
  );
}

import assert from "node:assert/strict";
import { parseSync } from "rolldown/experimental";

// This reviewed list is deliberately independent of future private members.
export const PLAYER_PRIVATE_PROPERTIES = [
  ["stockPipelines", "field"],
  ["stockPipelineTasks", "field"],
  ["opaqueDepthPipelines", "field"],
  ["windShadowPlanner", "field"],
  ["windShadowCopies", "field"],
  ["localSignatures", "field"],
  ["sourceBundleExecutions", "field"],
  ["culledSources", "field"],
  ["frameScenes", "field"],
  ["materialFeatures", "field"],
  ["distantFrameBuffer", "field"],
  ["renderMaterialInfo", "field"],
  ["submittedGeometry", "field"],
  ["activeCaptureActors", "field"],
  ["captureActorSourceIdentity", "field"],
  ["destroyedPlacements", "field"],
  ["stockPipelineFamily", "field"],
  ["authoredEntityDraws", "field"],
  ["pipelineDescriptor", "method"],
  ["rebuildFrameGroups", "method"],
  ["geometryVersion", "field"],
  ["sourceDebris", "field"],
  ["releaseBuffer", "method"],
  ["authoredInstances", "field"],
  ["cullingEnabled", "field"],
  ["clusteredLighting", "field"],
  ["shadowDirty", "field"],
  ["localFogViews", "field"],
  ["gameplayAnimationRevision", "field"],
  ["captureActorRegistry", "field"],
  ["stockChoiceRevision", "field"],
  ["materialPipeline", "method"],
  ["frameCompletions", "field"],
  ["fogAmbientVolume", "field"],
  ["gameplayEffects", "field"],
  ["captureReadback", "field"],
  ["captureCanvas", "field"],
  ["captureActorModelIdentity", "field"],
  ["environmentTextures", "field"],
  ["distantShadowMatrix", "field"],
  ["captureBuffer", "field"],
  ["gameplayDamageKeys", "field"],
  ["displayLutFallback", "field"],
  ["waterBounds", "field"],
  ["resolveMaterialPipeline", "method"],
  ["displayLuts", "field"],
  ["spellEffects", "field"],
  ["planarLightingRevision", "field"],
  ["occlusionFallback", "field"],
  ["invalidateCaptureViews", "method"],
  ["environmentGroups", "field"],
  ["displaySharpen", "field"],
  ["spellSwarmHost", "field"],
  ["desiredShader", "field"],
  ["fetchBuffer", "method"],
  ["enemyActors", "field"],
  ["localDirtyViews", "field"],
  ["farShadowMatrix", "field"],
  ["residentSource", "field"],
  ["planarEnvironmentSignature", "field"],
  ["materialEditor", "field"],
  ["localSlots", "field"],
  ["layerPools", "field"],
  ["canonicalDraws", "field"],
  ["planarEnvironmentRevision", "field"],
  ["windVertexBounded", "field"],
  ["waterPlaneHeights", "field"],
  ["planarFrameBuffer", "field"],
  ["meshSourceIndices", "field"],
] as const;
const names = new Set<string>(PLAYER_PRIVATE_PROPERTIES.map(([name]) => name));
const word = new RegExp(`\\b(?:${[...names].join("|")})\\b`);
const pipelines = [
  "pipeline",
  "blendPipeline",
  "additivePipeline",
  "planarPipeline",
  "planarBlendPipeline",
  "planarAdditivePipeline",
  "shadowPipeline",
  "depthPipeline",
];
export const playerPrivatePropertyMinify = () => ({
  include: new RegExp(`^(?:${[...names].join("|")})$`),
  quoted: false,
});
export interface PropertyModule {
  id: string;
  code: string;
}
type Node = Record<string, any>;
type Row = { node: Node; ancestors: Node[]; key: string };
function rowsOf(root: Node): Row[] {
  const rows: Row[] = [];
  function walk(node: unknown, ancestors: Node[] = [], key = "") {
    if (!node || typeof node !== "object") return;
    const value = node as Node;
    if (typeof value.type === "string")
      rows.push({ node: value, ancestors, key });
    for (const [name, child] of Object.entries(value)) {
      if (Array.isArray(child))
        for (const item of child) walk(item, [...ancestors, value], name);
      else if (child && typeof child === "object")
        walk(child, [...ancestors, value], name);
    }
  }
  walk(root);
  return rows;
}
function ast(module: PropertyModule): Node {
  const parsed = parseSync(module.id, module.code);
  assert.equal(
    parsed.errors.length,
    0,
    `Cannot audit player module: ${module.id}.`,
  );
  return parsed.program;
}
function member(node: Node, name: string) {
  return (
    node?.type === "MemberExpression" &&
    !node.computed &&
    node.property?.name === name
  );
}
function bindingNodes(pattern: Node | undefined): Node[] {
  if (!pattern) return [];
  if (pattern.type === "Identifier") return [pattern];
  if (pattern.type === "AssignmentPattern") return bindingNodes(pattern.left);
  if (pattern.type === "RestElement") return bindingNodes(pattern.argument);
  if (pattern.type === "ArrayPattern")
    return pattern.elements.flatMap(bindingNodes);
  if (pattern.type === "ObjectPattern")
    return pattern.properties.flatMap((property: Node) =>
      bindingNodes(
        property.type === "RestElement" ? property.argument : property.value,
      ),
    );
  return [];
}
function writesBinding(node: Node, name: string) {
  return (
    [
      "AssignmentExpression",
      "UpdateExpression",
      "ForOfStatement",
      "ForInStatement",
    ].includes(node.type) &&
    bindingNodes(node.left ?? node.argument).some(
      (binding) => binding.name === name,
    )
  );
}
function bindings(rows: Row[]) {
  return rows.flatMap(({ node, ancestors }) => {
    let patterns: Node[] = [];
    if (node.type === "VariableDeclarator") patterns = [node.id];
    else if (
      [
        "FunctionDeclaration",
        "FunctionExpression",
        "ArrowFunctionExpression",
      ].includes(node.type)
    )
      patterns = [...(node.id ? [node.id] : []), ...node.params];
    else if (
      ["ClassDeclaration", "ClassExpression"].includes(node.type) &&
      node.id
    )
      patterns = [node.id];
    else if (node.type === "CatchClause" && node.param) patterns = [node.param];
    else if (
      [
        "ImportSpecifier",
        "ImportDefaultSpecifier",
        "ImportNamespaceSpecifier",
      ].includes(node.type)
    )
      patterns = [node.local];
    return patterns.flatMap((pattern) =>
      bindingNodes(pattern).map((binding) => ({
        binding,
        owner: node,
        parent: ancestors.at(-1),
      })),
    );
  });
}
function constantStrings(rows: Row[]) {
  const declared = bindings(rows);
  const counts = new Map<string, number>();
  for (const item of declared)
    counts.set(item.binding.name, (counts.get(item.binding.name) ?? 0) + 1);
  const constants = new Map<string, Node>();
  for (const item of declared)
    if (
      item.owner.type === "VariableDeclarator" &&
      item.parent?.kind === "const" &&
      item.owner.id === item.binding &&
      counts.get(item.binding.name) === 1
    )
      constants.set(item.binding.name, item.owner.init);
  const unknown = Symbol();
  function value(node: Node | undefined, seen = new Set<string>()): unknown {
    if (!node) return unknown;
    if (
      node.type === "Literal" &&
      (node.value === null ||
        ["string", "number", "boolean"].includes(typeof node.value))
    )
      return node.value;
    if (
      [
        "TSAsExpression",
        "TSTypeAssertion",
        "TSNonNullExpression",
        "ParenthesizedExpression",
      ].includes(node.type)
    )
      return value(node.expression, seen);
    if (
      node.type === "Identifier" &&
      constants.has(node.name) &&
      !seen.has(node.name)
    )
      return value(constants.get(node.name), new Set([...seen, node.name]));
    if (node.type === "BinaryExpression" && node.operator === "+") {
      const a = value(node.left, seen),
        b = value(node.right, seen);
      if (a !== unknown && b !== unknown) return (a as any) + (b as any);
    }
    if (node.type === "TemplateLiteral") {
      let output = "";
      for (let index = 0; index < node.quasis.length; index++) {
        const cooked = node.quasis[index].value.cooked;
        if (typeof cooked !== "string") return unknown;
        output += cooked;
        if (index < node.expressions.length) {
          const expression = value(node.expressions[index], seen);
          if (expression === unknown) return unknown;
          output += String(expression);
        }
      }
      return output;
    }
    return unknown;
  }
  for (const { node } of rows) {
    const result = value(node);
    assert(
      typeof result !== "string" || !word.test(result),
      "A private player member has a cooked or constant string spelling.",
    );
  }
}
function rendererClass(program: Node): Node | undefined {
  return program.body.find(
    (node: Node) =>
      node.type === "ExportNamedDeclaration" &&
      node.declaration?.type === "ClassDeclaration" &&
      node.declaration.id?.name === "Renderer",
  )?.declaration;
}
function checkOccurrences(
  rows: Row[],
  cls: Node | undefined,
  declarations: Map<string, Node>,
) {
  for (const { node, ancestors, key } of rows) {
    if (node.type !== "Identifier" || !names.has(node.name)) continue;
    if (node === declarations.get(node.name)?.key) continue;
    const parent = ancestors.at(-1);
    assert(
      parent?.type === "MemberExpression" &&
        key === "property" &&
        !parent.computed &&
        parent.object?.type === "ThisExpression",
      `Non-this, computed, or object-key player member: ${node.name}.`,
    );
    assert(
      ancestors.findLast((item) =>
        ["ClassDeclaration", "ClassExpression"].includes(item.type),
      ) === cls && cls,
      `Private player member used outside Renderer: ${node.name}.`,
    );
    assert(
      !ancestors.some(
        (item) =>
          ["FunctionDeclaration", "FunctionExpression"].includes(item.type) &&
          !ancestors.some(
            (method) =>
              method.type === "MethodDefinition" && method.value === item,
          ),
      ),
      `Unknown this binding uses private player member: ${node.name}.`,
    );
  }
}
function checkIndirectAccess(rows: Row[], cls: Node) {
  const declared = bindings(rows);
  const lists = declared.filter(
    (item) => item.binding.name === "SHADER_PIPELINE_FIELDS",
  );
  assert(
    lists.length === 1 &&
      lists[0].owner.type === "VariableDeclarator" &&
      lists[0].parent?.kind === "const",
    "The indirect pipeline binding changed or is shadowed.",
  );
  const initializer = lists[0].owner.init;
  const array =
    initializer?.type === "TSAsExpression"
      ? initializer.expression
      : initializer;
  assert(
    array?.type === "ArrayExpression",
    "The indirect pipeline list must be explicit.",
  );
  assert.deepEqual(
    array.elements.map((element: Node) =>
      element.type === "Literal" ? element.value : null,
    ),
    pipelines,
    "Review changes to indirect pipeline fields before shortening private members.",
  );
  for (const { node, ancestors } of rows) {
    if (
      node.type !== "Identifier" ||
      node.name !== "SHADER_PIPELINE_FIELDS" ||
      node === lists[0].binding
    )
      continue;
    const parent = ancestors.at(-1),
      call = ancestors.at(-2);
    assert(
      parent?.type === "MemberExpression" &&
        parent.object === node &&
        (member(parent, "map") || member(parent, "forEach")) &&
        call?.type === "CallExpression" &&
        call.callee === parent,
      "The fixed indirect pipeline list may not be mutated or aliased.",
    );
  }
  for (const { node, ancestors } of rows) {
    if (
      ancestors.findLast((item) =>
        ["ClassDeclaration", "ClassExpression"].includes(item.type),
      ) !== cls
    )
      continue;
    const parent = ancestors.at(-1);
    if (
      node.type === "MemberExpression" &&
      node.computed &&
      node.object?.type === "ThisExpression"
    ) {
      const callback = ancestors.findLast(
        (item) => item.type === "ArrowFunctionExpression",
      );
      const call = ancestors.findLast(
        (item) =>
          item.type === "CallExpression" && item.arguments.includes(callback),
      );
      assert(
        callback && call,
        "The computed Renderer access must use its fixed callback.",
      );
      assert(
        node.property.type === "Identifier" &&
          node.property.name === "field" &&
          call &&
          (member(call.callee, "map") || member(call.callee, "forEach")) &&
          call.callee.object?.name === "SHADER_PIPELINE_FIELDS" &&
          callback.params[0]?.type === "Identifier" &&
          callback.params[0].name === "field",
        "Unaudited computed Renderer access.",
      );
      const local = bindings(rowsOf(callback)).filter(
        (item) => item.binding.name === "field",
      );
      assert(
        local.length === 1 && local[0].binding === callback.params[0],
        "The indirect field parameter is shadowed.",
      );
      for (const { node: item } of rowsOf(callback))
        assert(
          !writesBinding(item, "field"),
          "The indirect field parameter is reassigned.",
        );
    }
    if (
      node.type !== "ThisExpression" ||
      (parent?.type === "MemberExpression" && parent.object === node) ||
      parent?.type === "TSQualifiedName"
    )
      continue;
    assert(
      parent?.type === "CallExpression" &&
        parent.arguments[0] === node &&
        member(parent.callee, "assign") &&
        parent.callee.object?.name === "Object" &&
        parent.arguments.length === 2 &&
        parent.arguments[1]?.name === "selected" &&
        ancestors.at(-2)?.type === "ExpressionStatement" &&
        ancestors.at(-2)?.expression === parent,
      "Renderer this may not escape through a value, reflection, or a returned assignment.",
    );
    assert(
      !declared.some((item) => item.binding.name === "Object"),
      "The Object binding is shadowed.",
    );
    const method = ancestors.findLast(
      (item) => item.type === "MethodDefinition",
    );
    assert(method, "The fixed Renderer assignment must belong to its method.");
    const methodRows = rowsOf(method);
    const selected = bindings(methodRows).filter(
      (item) => item.binding.name === "selected",
    );
    assert(
      selected.length === 1 &&
        selected[0].owner.type === "VariableDeclarator" &&
        selected[0].parent?.kind === "const",
      "The fixed Renderer assignment binding changed or is shadowed.",
    );
    const object = selected[0].owner.init;
    assert(
      object?.type === "ObjectExpression",
      "The Renderer assignment keys must be explicit.",
    );
    for (const { node: item } of methodRows)
      assert(
        !writesBinding(item, "selected"),
        "The fixed Renderer assignment binding is reassigned.",
      );
    assert.deepEqual(
      object.properties
        .map((property: Node) => {
          assert(
            property.type === "Property" &&
              !property.computed &&
              property.key.type === "Identifier" &&
              member(property.value, property.key.name) &&
              property.value.object.type === "ThisExpression",
            "Unknown Renderer assignment key or value.",
          );
          return property.key.name;
        })
        .sort(),
      ["draws", "instances", "lamps", "triangles"],
      "The fixed Renderer assignment keys changed.",
    );
  }
}

/**
 * Checks this explicit ownership convention, not arbitrary JavaScript reflection or aliasing.
 * Every included module is parsed. Known direct/constant spellings and Renderer-this escapes
 * fail the build; runtime-generated names and external reflection remain unsupported.
 */
export function auditPlayerPrivateProperties(input: {
  renderer: PropertyModule;
  modules: readonly PropertyModule[];
  emitted: readonly PropertyModule[];
  mappings: Readonly<Record<string, string | false>>;
}) {
  const program = ast(input.renderer),
    rows = rowsOf(program),
    cls = rendererClass(program);
  assert(cls, "Expected the owned Renderer source.");
  const declarations = new Map<string, Node>();
  for (const [name, kind] of PLAYER_PRIVATE_PROPERTIES) {
    const matches = cls.body.body.filter(
      (node: Node) => node.key?.name === name,
    );
    assert(
      matches.length === 1,
      `Missing or duplicate private player member: ${name}.`,
    );
    const declaration = matches[0];
    assert(
      declaration.accessibility === "private" &&
        !declaration.computed &&
        !declaration.static &&
        declaration.type ===
          (kind === "field" ? "PropertyDefinition" : "MethodDefinition") &&
        (kind === "field" || declaration.kind === "method"),
      `Private player member kind or visibility changed: ${name}.`,
    );
    declarations.set(name, declaration);
  }
  checkOccurrences(rows, cls, declarations);
  checkIndirectAccess(rows, cls);
  constantStrings(rows);
  let rendererIncluded = false;
  for (const module of input.modules) {
    const parsed = ast(module),
      includedRows = rowsOf(parsed);
    const owned = module.id.split("?")[0] === input.renderer.id;
    const includedClass = owned ? rendererClass(parsed) : undefined;
    const includedDeclarations = new Map<string, Node>();
    if (owned) {
      rendererIncluded = true;
      assert(includedClass, "The included Renderer class changed shape.");
      for (const node of includedClass.body.body)
        if (names.has(node.key?.name))
          includedDeclarations.set(node.key.name, node);
      checkIndirectAccess(includedRows, includedClass);
    }
    checkOccurrences(includedRows, includedClass, includedDeclarations);
    constantStrings(includedRows);
  }
  assert.equal(
    input.emitted.length,
    1,
    "Private-property player builds require one JavaScript chunk.",
  );
  for (const module of input.emitted) {
    const emittedRows = rowsOf(ast(module));
    checkOccurrences(emittedRows, undefined, new Map());
    constantStrings(emittedRows);
  }
  const keys = Object.keys(input.mappings);
  assert(
    keys.every((name) => names.has(name)),
    "The minifier renamed a member outside the explicit list.",
  );
  assert(
    keys.length === (rendererIncluded ? names.size : 0),
    "The minifier did not apply the expected explicit private list.",
  );
  const targets = Object.values(input.mappings);
  assert(
    targets.every(
      (target) =>
        typeof target === "string" &&
        /^[A-Za-z_$][\w$]*$/.test(target) &&
        !names.has(target) &&
        !["__proto__", "constructor", "prototype"].includes(target),
    ),
    "Invalid private-property minifier mapping.",
  );
  assert.equal(
    new Set(targets).size,
    targets.length,
    "Private-property minifier targets must be unique.",
  );
  return {
    schema: "aethyrforge.player-private-properties.v1",
    includedModules: input.modules.length,
    emittedChunks: input.emitted.length,
    rendererIncluded,
    names: PLAYER_PRIVATE_PROPERTIES,
    mappings: input.mappings,
    scope:
      "Explicit owned Renderer member convention; not a proof against arbitrary reflection or aliases.",
  };
}

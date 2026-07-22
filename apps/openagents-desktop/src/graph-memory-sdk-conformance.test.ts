import {
  runDseExtractionLaws,
  runGraphArchiveLaws,
  runGraphCapabilityDeleteLaws,
  runGraphIdentityLaws,
  runGraphRankingLaws,
  runGraphRlmLaws,
} from "@openagentsinc/conformance-kit";
import {
  buildGraphCorpus,
  makeCanonicalEntity,
  makeCompleteGraphDeleteExecutionResult,
  makeEmbeddingProjectionDescriptor,
  makeGraphAdapterCapabilities,
  makeGraphArtifactInventory,
  makeGraphDeleteReceipt,
  makeGraphMention,
  makeGraphRelation,
  makeGraphRlmClassificationProjection,
  makeGraphRlmProjection,
  makeInMemoryGraphSnapshotHandle,
  makeMergeEvidence,
  planGraphSourceDeletion,
  requireExecutableGraphDeletePlan,
  requireGraphAdapterCapability,
  validateGraphDeleteExecutionResult,
  validateGraphDeleteReceipt,
  verifyBuiltGraphCorpus,
} from "@openagentsinc/graph-corpus";
import {
  encodeGraphCorpusArchive,
  exportGraphCorpusArchive,
  importGraphCorpusArchive,
} from "@openagentsinc/graph-corpus/archive";
import {
  makeGraphFeedbackObservation,
  makeGraphRankingSnapshot,
  rankGraphOperationResult,
  validateGraphRankingSnapshot,
  validateGraphUsedElementEvidence,
} from "@openagentsinc/graph-corpus/ranking";
import {
  applyGraphExtractionCandidates,
  planGraphExtractionBatches,
  runDeterministicGraphExtraction,
  runGraphExtraction,
  validateGraphExtractionRunReceipt,
} from "@openagentsinc/dse";

const label = "OpenAgents Desktop SDK train 0.2.1-rc.2";

runGraphIdentityLaws({
  label,
  buildGraphCorpus,
  verifyBuiltGraphCorpus,
  makeGraphMention,
  makeCanonicalEntity,
  makeGraphRelation,
  makeMergeEvidence,
});

runGraphCapabilityDeleteLaws({
  label,
  buildGraphCorpus,
  makeGraphMention,
  makeCanonicalEntity,
  makeGraphAdapterCapabilities,
  requireGraphAdapterCapability,
  makeGraphArtifactInventory,
  planGraphSourceDeletion,
  requireExecutableGraphDeletePlan,
  makeCompleteGraphDeleteExecutionResult,
  validateGraphDeleteExecutionResult,
  makeGraphDeleteReceipt,
  validateGraphDeleteReceipt,
});

runDseExtractionLaws({
  label,
  planGraphExtractionBatches,
  runGraphExtraction,
  runDeterministicGraphExtraction,
  validateGraphExtractionRunReceipt,
  applyGraphExtractionCandidates,
});

runGraphRlmLaws({
  label,
  buildGraphCorpus,
  makeGraphMention,
  makeInMemoryGraphSnapshotHandle,
  makeGraphRlmClassificationProjection,
  makeGraphAdapterCapabilities,
  makeGraphRlmProjection,
});

runGraphRankingLaws({
  label,
  buildGraphCorpus,
  makeGraphMention,
  makeCanonicalEntity,
  makeEmbeddingProjectionDescriptor,
  makeInMemoryGraphSnapshotHandle,
  makeGraphRlmClassificationProjection,
  makeGraphAdapterCapabilities,
  makeGraphRlmProjection,
  makeGraphRankingSnapshot,
  makeGraphFeedbackObservation,
  validateGraphRankingSnapshot,
  rankGraphOperationResult,
  validateGraphUsedElementEvidence,
});

runGraphArchiveLaws({
  label,
  buildGraphCorpus,
  makeGraphMention,
  makeGraphAdapterCapabilities,
  makeGraphArtifactInventory,
  exportGraphCorpusArchive,
  encodeGraphCorpusArchive,
  importGraphCorpusArchive,
});

// Verification file to ensure MLX LLM dependencies are accessible.
// Safe to remove once GPTOSSAgentProvider is implemented.

#if os(macOS)
import MLX
import MLXLLM
import MLXLMCommon
import MLXNN
import Tokenizers

@usableFromInline
func _verifyMLXLLMImports() {
    _ = Optional<ModelConfiguration>.none
}
#endif

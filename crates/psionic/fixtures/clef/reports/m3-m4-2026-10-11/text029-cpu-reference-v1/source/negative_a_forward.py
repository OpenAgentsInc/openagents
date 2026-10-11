def forward(
    self,
    hidden_states: torch.Tensor,
    cache_params: Cache | None = None,
    attention_mask: torch.Tensor | None = None,
    **kwargs: Unpack[TransformersKwargs],
):
    hidden_states = apply_mask_to_padding_states(hidden_states, attention_mask)

    # Set up dimensions for reshapes later
    batch_size, seq_len, _ = hidden_states.shape

    # We have cached `conv_state` / `recurrent_state` to continue from. The two cached modes
    # (single-token decode and chunk-tokens continuation) share the state read here; they only
    # diverge in how the conv input is assembled and which kernel consumes the states below,
    # which we gate locally on `seq_len`.
    use_precomputed_states = cache_params is not None and cache_params.has_previous_state(self.layer_idx)

    # getting projected states from cache if it exists
    if use_precomputed_states:
        conv_state = cache_params.layers[self.layer_idx].conv_states
        recurrent_state = cache_params.layers[self.layer_idx].recurrent_states

    mixed_qkv = self.in_proj_qkv(hidden_states)
    mixed_qkv = mixed_qkv.transpose(1, 2)

    z = self.in_proj_z(hidden_states)
    z = z.reshape(batch_size, seq_len, -1, self.head_v_dim)

    b = self.in_proj_b(hidden_states)
    a = self.in_proj_a(hidden_states)

    if use_precomputed_states and seq_len == 1:
        # Single-token cached decode: the fused per-step kernel updates the conv state in-place.
        mixed_qkv = self.causal_conv1d_update(
            mixed_qkv,
            conv_state,
            self.conv1d.weight.squeeze(1),
            self.conv1d.bias,
            self.activation,
        )
    else:
        # Multi-token forward (prefill, or chunked-tokens decode when the cache has prior state).
        if use_precomputed_states:
            # Cached chunked-tokens decode: prepend the cached conv context so the causal conv
            # sees the correct left-context rather than zero-padding. Dropped from the output
            # at the end of this branch.
            mixed_qkv = torch.cat([conv_state, mixed_qkv], dim=-1)
        if cache_params is not None:
            new_conv_state = F.pad(mixed_qkv, (self.conv_kernel_size - mixed_qkv.shape[-1], 0))
            cache_params.update_conv_state(new_conv_state, self.layer_idx)
        if self.causal_conv1d_fn is not None:
            mixed_qkv = self.causal_conv1d_fn(
                x=mixed_qkv,
                weight=self.conv1d.weight.squeeze(1),
                bias=self.conv1d.bias,
                activation=self.activation,
                seq_idx=kwargs.get("seq_idx"),
            )
        else:
            mixed_qkv = F.silu(self.conv1d(mixed_qkv)[:, :, : mixed_qkv.shape[-1]])
        if use_precomputed_states:
            mixed_qkv = mixed_qkv[:, :, -seq_len:]

    mixed_qkv = mixed_qkv.transpose(1, 2)
    query, key, value = torch.split(
        mixed_qkv,
        [
            self.key_dim,
            self.key_dim,
            self.value_dim,
        ],
        dim=-1,
    )

    query = query.reshape(batch_size, seq_len, -1, self.head_k_dim)
    key = key.reshape(batch_size, seq_len, -1, self.head_k_dim)
    value = value.reshape(batch_size, seq_len, -1, self.head_v_dim)

    beta = b.sigmoid()
    # If the model is loaded in fp16, without the .float() here, A might be -inf
    g = self.gguf_negative_a.float() * F.softplus(a.float() + self.dt_bias)
    if self.num_v_heads // self.num_k_heads > 1:
        query = query.repeat_interleave(self.num_v_heads // self.num_k_heads, dim=2)
        key = key.repeat_interleave(self.num_v_heads // self.num_k_heads, dim=2)

    if use_precomputed_states and seq_len == 1:
        core_attn_out, last_recurrent_state = self.recurrent_gated_delta_rule(
            query,
            key,
            value,
            g=g,
            beta=beta,
            initial_state=recurrent_state,
            output_final_state=cache_params is not None,
            use_qk_l2norm_in_kernel=True,
        )
    else:
        core_attn_out, last_recurrent_state = self.chunk_gated_delta_rule(
            query,
            key,
            value,
            g=g,
            beta=beta,
            initial_state=recurrent_state if use_precomputed_states else None,
            output_final_state=cache_params is not None,
            use_qk_l2norm_in_kernel=True,
            # The chunked FLA kernel takes a single `cu_seqlens` arg; for packed self-attention this matches q-side lengths.
            cu_seqlens=kwargs.get("cu_seq_lens_q"),
        )

    # Update cache
    if cache_params is not None:
        cache_params.update_recurrent_state(last_recurrent_state, self.layer_idx)

    # reshape input data into 2D tensor
    core_attn_out = core_attn_out.reshape(-1, self.head_v_dim)
    z = z.reshape(-1, self.head_v_dim)
    core_attn_out = self.norm(core_attn_out, z)
    core_attn_out = core_attn_out.reshape(batch_size, seq_len, -1)

    output = self.out_proj(core_attn_out)
    return output

use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum gptoss_status {
    gptoss_status_success = 0,
    gptoss_status_invalid_argument = 1,
    gptoss_status_unsupported_argument = 2,
    gptoss_status_invalid_state = 3,
    gptoss_status_io_error = 4,
    gptoss_status_insufficient_memory = 5,
    gptoss_status_insufficient_resources = 6,
    gptoss_status_unsupported_system = 7,
    gptoss_status_context_overflow = 8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum gptoss_special_token {
    gptoss_special_token_invalid = 0,
    gptoss_special_token_return = 1,
    gptoss_special_token_start = 2,
    gptoss_special_token_message = 3,
    gptoss_special_token_end = 4,
    gptoss_special_token_refusal = 5,
    gptoss_special_token_constrain = 6,
    gptoss_special_token_channel = 7,
    gptoss_special_token_call = 8,
    gptoss_special_token_untrusted = 9,
    gptoss_special_token_end_untrusted = 10,
    gptoss_special_token_max = 11,
}

#[repr(C)]
pub struct gptoss_model {
    _private: [u8; 0],
}

#[repr(C)]
pub struct gptoss_tokenizer {
    _private: [u8; 0],
}

#[repr(C)]
pub struct gptoss_context {
    _private: [u8; 0],
}

#[repr(C)]
pub struct gptoss_sampler {
    _private: [u8; 0],
}

pub type gptoss_model_t = *mut gptoss_model;
pub type gptoss_tokenizer_t = *mut gptoss_tokenizer;
pub type gptoss_context_t = *mut gptoss_context;
pub type gptoss_sampler_t = *mut gptoss_sampler;

extern "C" {
    pub fn gptoss_model_create_from_file(
        path: *const c_char,
        model_out: *mut gptoss_model_t,
    ) -> gptoss_status;

    pub fn gptoss_model_get_tokenizer(
        model: gptoss_model_t,
        tokenizer_out: *mut gptoss_tokenizer_t,
    ) -> gptoss_status;

    pub fn gptoss_model_get_max_context_length(
        model: gptoss_model_t,
        max_context_length_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_model_retain(model: gptoss_model_t) -> gptoss_status;

    pub fn gptoss_model_release(model: gptoss_model_t) -> gptoss_status;

    pub fn gptoss_tokenizer_get_special_token_id(
        tokenizer: gptoss_tokenizer_t,
        token_type: gptoss_special_token,
        token_id_out: *mut u32,
    ) -> gptoss_status;

    pub fn gptoss_tokenizer_get_num_text_tokens(
        tokenizer: gptoss_tokenizer_t,
        num_text_tokens_out: *mut u32,
    ) -> gptoss_status;

    pub fn gptoss_tokenizer_get_num_special_tokens(
        tokenizer: gptoss_tokenizer_t,
        num_special_tokens_out: *mut u32,
    ) -> gptoss_status;

    pub fn gptoss_tokenizer_get_num_tokens(
        tokenizer: gptoss_tokenizer_t,
        num_tokens_out: *mut u32,
    ) -> gptoss_status;

    pub fn gptoss_tokenizer_decode(
        tokenizer: gptoss_tokenizer_t,
        token_id: u32,
        token_ptr_out: *mut *const c_void,
        token_size_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_tokenizer_retain(tokenizer: gptoss_tokenizer_t) -> gptoss_status;

    pub fn gptoss_tokenizer_release(tokenizer: gptoss_tokenizer_t) -> gptoss_status;

    pub fn gptoss_context_create(
        model: gptoss_model_t,
        context_length: usize,
        max_batch_tokens: usize,
        context_out: *mut gptoss_context_t,
    ) -> gptoss_status;

    pub fn gptoss_context_get_num_tokens(
        context: gptoss_context_t,
        num_tokens_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_context_get_max_tokens(
        context: gptoss_context_t,
        max_tokens_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_context_get_tokens(
        context: gptoss_context_t,
        tokens_out: *mut u32,
        max_tokens: usize,
        num_tokens_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_context_append_chars(
        context: gptoss_context_t,
        text: *const c_char,
        text_length: usize,
        num_tokens_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_context_append_tokens(
        context: gptoss_context_t,
        num_tokens: usize,
        tokens: *const u32,
    ) -> gptoss_status;

    pub fn gptoss_context_reset(context: gptoss_context_t) -> gptoss_status;

    pub fn gptoss_context_process(context: gptoss_context_t) -> gptoss_status;

    pub fn gptoss_context_sample(
        context: gptoss_context_t,
        temperature: f32,
        seed: u64,
        max_tokens: usize,
        tokens_out: *mut u32,
        num_tokens_out: *mut usize,
    ) -> gptoss_status;

    pub fn gptoss_context_retain(context: gptoss_context_t) -> gptoss_status;

    pub fn gptoss_context_release(context: gptoss_context_t) -> gptoss_status;

    pub fn gptoss_sampler_create(sampler_out: *mut gptoss_sampler_t) -> gptoss_status;

    pub fn gptoss_sampler_set_temperature(
        sampler: gptoss_sampler_t,
        temperature: f32,
    ) -> gptoss_status;

    pub fn gptoss_sampler_set_top_p(sampler: gptoss_sampler_t, top_p: f32) -> gptoss_status;

    pub fn gptoss_sampler_set_presence_penalty(
        sampler: gptoss_sampler_t,
        presence_penalty: f32,
    ) -> gptoss_status;

    pub fn gptoss_sampler_set_frequency_penalty(
        sampler: gptoss_sampler_t,
        frequency_penalty: f32,
    ) -> gptoss_status;

    pub fn gptoss_sampler_retain(sampler: gptoss_sampler_t) -> gptoss_status;

    pub fn gptoss_sampler_release(sampler: gptoss_sampler_t) -> gptoss_status;
}

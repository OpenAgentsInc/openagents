pub(crate) mod frontmatter;
pub(crate) mod prompt;

pub(crate) use frontmatter::{
    first_nonempty_line, frontmatter_list, frontmatter_scalar, parse_frontmatter, Frontmatter,
};
pub(crate) use prompt::{build_context_injection, build_todo_context, expand_prompt_text};

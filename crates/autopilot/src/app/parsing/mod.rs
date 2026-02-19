pub(crate) mod frontmatter;
pub(crate) mod prompt;

pub(crate) use frontmatter::{
    Frontmatter, first_nonempty_line, frontmatter_list, frontmatter_scalar, parse_frontmatter,
};

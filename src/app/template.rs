use super::port::RepositoryResult;

pub(crate) fn render_template<'a, F>(template: &str, mut resolve: F) -> RepositoryResult<String>
where
    F: FnMut(&str) -> Option<&'a str>,
{
    let mut rendered = String::new();
    let mut remaining = template;

    loop {
        let Some(start) = remaining.find('{') else {
            if remaining.contains('}') {
                return Err("unmatched closing brace".into());
            }
            rendered.push_str(remaining);
            break;
        };

        if remaining[..start].contains('}') {
            return Err("unmatched closing brace".into());
        }
        rendered.push_str(&remaining[..start]);
        let end = remaining[start..]
            .find('}')
            .map(|offset| start + offset)
            .ok_or("unterminated placeholder")?;
        let name = &remaining[start + 1..end];
        let value = resolve(name).ok_or_else(|| format!("unknown placeholder: {{{name}}}"))?;

        rendered.push_str(value);
        remaining = &remaining[end + 1..];
    }

    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_each_placeholder_once() {
        let result = render_template("{repo}/{branch}/{custom}", |name| match name {
            "repo" => Some("repo-{custom}"),
            "branch" => Some("feature/test"),
            "custom" => Some("value"),
            _ => None,
        })
        .unwrap();

        assert_eq!(result, "repo-{custom}/feature/test/value");
    }

    #[test]
    fn rejects_unknown_placeholder() {
        let result = render_template("{unknown}", |_| None);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_unterminated_placeholder() {
        let result = render_template("{repo", |_| Some("repo"));

        assert!(result.is_err());
    }

    #[test]
    fn rejects_unmatched_closing_brace() {
        let result = render_template("xxx}", |_| None);

        assert!(result.is_err());
    }
}

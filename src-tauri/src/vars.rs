use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
#[error("unresolved variable ${{{0}}}")]
pub struct UnresolvedVariable(pub String);

/// Resolves `${NAME}` placeholders against `variables` first, falling back
/// to the process environment (so `${HOME}` / `${USERPROFILE}` work without
/// the user having to redeclare them per workspace).
pub fn resolve(
    input: &str,
    variables: &HashMap<String, String>,
) -> Result<String, UnresolvedVariable> {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find("${") {
        let Some(end_rel) = rest[start..].find('}') else {
            out.push_str(rest);
            return Ok(out);
        };
        let end = start + end_rel;
        out.push_str(&rest[..start]);

        let name = &rest[start + 2..end];
        let value = variables
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok())
            .ok_or_else(|| UnresolvedVariable(name.to_string()))?;
        out.push_str(&value);

        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_from_variables_map() {
        let mut vars = HashMap::new();
        vars.insert("PROJECT_DIR".to_string(), "C:/dev/project-x".to_string());
        assert_eq!(
            resolve("${PROJECT_DIR}/web", &vars).unwrap(),
            "C:/dev/project-x/web"
        );
    }

    #[test]
    fn falls_back_to_process_env() {
        let vars = HashMap::new();
        std::env::set_var("CLICK_TEST_VAR", "value123");
        assert_eq!(resolve("${CLICK_TEST_VAR}", &vars).unwrap(), "value123");
    }

    #[test]
    fn errors_on_unresolved_variable() {
        let vars = HashMap::new();
        let err = resolve("${NOPE_NOT_SET_ANYWHERE}", &vars).unwrap_err();
        assert_eq!(err.0, "NOPE_NOT_SET_ANYWHERE");
    }

    #[test]
    fn passes_through_plain_text() {
        let vars = HashMap::new();
        assert_eq!(
            resolve("no variables here", &vars).unwrap(),
            "no variables here"
        );
    }
}

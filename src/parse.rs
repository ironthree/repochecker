use crate::data::{BrokenDep, Package};

#[allow(clippy::many_single_char_names)]
pub fn parse_nevra(nevra: &str) -> Result<(&str, &str, &str, &str, &str), String> {
    let mut nevr_a: Vec<&str> = nevra.rsplitn(2, '.').collect();

    if nevr_a.len() != 2 {
        return Err(format!("Unexpected error when parsing NEVRAs: {}", nevra));
    };

    // rsplitn returns things in reverse order
    let a = nevr_a.remove(0);
    let nevr = nevr_a.remove(0);

    let mut n_ev_r: Vec<&str> = nevr.rsplitn(3, '-').collect();

    if n_ev_r.len() != 3 {
        return Err(format!("Unexpected error when parsing NEVRAs: {}", nevr));
    };

    // rsplitn returns things in reverse order
    let r = n_ev_r.remove(0);
    let ev = n_ev_r.remove(0);
    let n = n_ev_r.remove(0);

    let (e, v) = if ev.contains(':') {
        let mut e_v: Vec<&str> = ev.split(':').collect();
        let e = e_v.remove(0);
        let v = e_v.remove(0);
        (e, v)
    } else {
        ("0", ev)
    };

    Ok((n, e, v, r, a))
}

pub(crate) fn parse_repoquery(string: &str) -> Result<Vec<Package>, String> {
    let lines = string.split('\n');

    let mut packages: Vec<Package> = Vec::new();
    for line in lines {
        let mut split = line.split(' ');

        // match only exactly 6 components
        match (
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
        ) {
            (Some(name), Some(source), Some(epoch), Some(version), Some(release), Some(arch), None) => {
                packages.push(Package {
                    name: name.to_string(),
                    source_name: source.to_string(),
                    epoch: match epoch.parse() {
                        Ok(value) => value,
                        Err(error) => return Err(format!("Failed to parse Epoch value: {}", error)),
                    },
                    version: version.to_string(),
                    release: release.to_string(),
                    arch: arch.to_string(),
                })
            },
            _ => return Err(format!("Failed to parse line: {}", line)),
        };
    }

    Ok(packages)
}

#[allow(clippy::many_single_char_names)]
pub(crate) fn parse_repoclosure(string: &str) -> Result<Vec<BrokenDep>, String> {
    let lines = string.split('\n');

    let mut broken_deps: Vec<BrokenDep> = Vec::new();

    struct State<'a> {
        nevra: (&'a str, &'a str, &'a str, &'a str, &'a str),
        repo: &'a str,
        broken: Vec<&'a str>,
    };

    let state_to_dep = |state: State| -> Result<BrokenDep, String> {
        let (n, e, v, r, a) = state.nevra;

        Ok(BrokenDep {
            package: n.to_string(),
            epoch: e.to_string(),
            version: v.to_string(),
            release: r.to_string(),
            arch: a.to_string(),
            repo: state.repo.to_string(),
            broken: state.broken.iter().map(|s| s.to_string()).collect(),
            repo_arch: None,
            source: None,
            admin: None,
        })
    };

    let mut state: Option<State> = None;

    for line in lines {
        if line.starts_with("package: ") {
            if let Some(status) = state {
                broken_deps.push(state_to_dep(status)?);
            }

            let mut split = line.split(' ');
            match (split.next(), split.next(), split.next(), split.next()) {
                (Some(_), Some(nevra), Some(_), Some(repo)) => {
                    state = Some(State {
                        nevra: parse_nevra(nevra)?,
                        repo,
                        broken: Vec::new(),
                    });
                },
                _ => return Err(format!("Failed to parse line from repoclosure output: {}", line)),
            }
        } else if line.starts_with("  unresolved deps:") {
            continue;
        } else if line.starts_with("    ") {
            match &mut state {
                Some(state) => state.broken.push(line.trim()),
                None => return Err(String::from("Unrecognised output from repoclosure.")),
            };
        } else {
            continue;
        }
    }

    // this should always be true
    if let Some(status) = state {
        broken_deps.push(state_to_dep(status)?);
    }

    Ok(broken_deps)
}

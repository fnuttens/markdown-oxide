use std::path::Path;

use tower_lsp::lsp_types::{Position, Location, Url};

use crate::vault::{Vault, Referenceable};

pub fn references(vault: &Vault, cursor_position: Position, path: &Path) -> Option<Vec<Location>> {
    // First we need to get the linkable node under the cursor
    let path = path.to_path_buf();
    let linkable_nodes = vault.select_linkable_nodes_for_path(&path)?;
    let linkable = linkable_nodes
        .iter()
        .find(|&l| 
            l.get_range().start.line <= cursor_position.line && 
            l.get_range().end.line >= cursor_position.line && 
            l.get_range().start.character <= cursor_position.character &&
            l.get_range().end.character >= cursor_position.character
        )?;


    let references = vault.select_references(None)?;
    let locations = |reference_text| references.iter()
        .filter(move |r| r.1.reference_text == reference_text)
        .map(|link| Url::from_file_path(link.0).map(|good| Location {uri: good, range: link.1.range}))
        .flat_map(|l| match l.is_ok() {
            true => Some(l),
            false => None
        })
        .flatten();

    return match linkable {
        Referenceable::File(_, _) => {
            return Some(linkable_nodes.iter()
                .filter(|&referenceable| !matches!(referenceable, &Referenceable::Tag(_, _)))
                .filter_map(|linkable| linkable.get_refname(vault.root_dir()))
                .map(|refname| locations(refname))
                .flatten()
                .collect())
        }
        _ => linkable.get_refname(vault.root_dir()).and_then(|r| Some(locations(r).collect()))
    }
}

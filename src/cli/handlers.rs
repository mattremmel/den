//! Command handlers (stubs).

use anyhow::Result;

use super::{
    BacklinksArgs, CheckArgs, EditArgs, IndexArgs, LinkArgs, ListArgs, NewArgs, RelsArgs,
    SearchArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UnlinkArgs, UntagArgs,
};

pub fn handle_index(_args: &IndexArgs) -> Result<()> {
    println!("index: not yet implemented");
    Ok(())
}

pub fn handle_list(_args: &ListArgs) -> Result<()> {
    println!("ls: not yet implemented");
    Ok(())
}

pub fn handle_search(_args: &SearchArgs) -> Result<()> {
    println!("search: not yet implemented");
    Ok(())
}

pub fn handle_new(_args: &NewArgs) -> Result<()> {
    println!("new: not yet implemented");
    Ok(())
}

pub fn handle_show(_args: &ShowArgs) -> Result<()> {
    println!("show: not yet implemented");
    Ok(())
}

pub fn handle_edit(_args: &EditArgs) -> Result<()> {
    println!("edit: not yet implemented");
    Ok(())
}

pub fn handle_topics(_args: &TopicsArgs) -> Result<()> {
    println!("topics: not yet implemented");
    Ok(())
}

pub fn handle_tags(_args: &TagsArgs) -> Result<()> {
    println!("tags: not yet implemented");
    Ok(())
}

pub fn handle_tag(_args: &TagArgs) -> Result<()> {
    println!("tag: not yet implemented");
    Ok(())
}

pub fn handle_untag(_args: &UntagArgs) -> Result<()> {
    println!("untag: not yet implemented");
    Ok(())
}

pub fn handle_check(_args: &CheckArgs) -> Result<()> {
    println!("check: not yet implemented");
    Ok(())
}

pub fn handle_backlinks(_args: &BacklinksArgs) -> Result<()> {
    println!("backlinks: not yet implemented");
    Ok(())
}

pub fn handle_link(_args: &LinkArgs) -> Result<()> {
    println!("link: not yet implemented");
    Ok(())
}

pub fn handle_unlink(_args: &UnlinkArgs) -> Result<()> {
    println!("unlink: not yet implemented");
    Ok(())
}

pub fn handle_rels(_args: &RelsArgs) -> Result<()> {
    println!("rels: not yet implemented");
    Ok(())
}

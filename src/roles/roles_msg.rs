use crate::{roles::WereWolfRoleConfig, Reactions};

pub fn get_roles_msg(roles: &[WereWolfRoleConfig]) -> String {
    let mut msg = "Select all Roles to use:\n".to_string();
    for role in roles {
        msg.push_str(&format!("{}: {}\n", role.emoji(), role.name()));
    }
    msg.push_str(&format!(
        "Use {} and {} to navigate between the Pages",
        Reactions::PreviousPage,
        Reactions::NextPage,
    ));
    msg
}

use crate::{roles::WereWolfRole, Reactions};

pub fn get_roles_msg(roles: &[WereWolfRole]) -> String {
    let mut msg = "Select all Roles to use:\n".to_string();
    for role in roles {
        let emoji = role.to_emoji();

        msg.push_str(&format!("{}: {}\n", emoji, role));
    }
    msg.push_str(&format!(
        "Use {} and {} to navigate between the Pages",
        Reactions::PreviousPage,
        Reactions::NextPage,
    ));
    msg
}

use std::collections::BTreeMap;

use rand::Rng;
use serenity::model::id::UserId;

use super::WereWolfRole;

fn populate_nested_roles<R>(mut roles: Vec<WereWolfRole>, rng: &mut R) -> Vec<WereWolfRole>
where
    R: Rng,
{
    loop {
        let index_result = roles
            .iter()
            .enumerate()
            .find(|(_, tmp_r)| tmp_r.needs_other_role());

        let index = match index_result {
            Some((i, _)) => i,
            None => break,
        };

        let tmp_role = roles.remove(index);

        let other_role_index: usize = rng.gen::<usize>() % roles.len();
        let other_role = roles.remove(other_role_index);

        let new_role = match tmp_role {
            WereWolfRole::Trunkenbold(_) => WereWolfRole::Trunkenbold(Some(Box::new(other_role))),
            _ => panic!("Unexpected Nested-Role"),
        };

        roles.push(new_role);
    }

    roles
}

/// This will actually distribute the Roles among the Players
fn distribute<R>(
    participants: Vec<UserId>,
    roles: BTreeMap<WereWolfRole, usize>,
    rng: &mut R,
) -> Result<BTreeMap<UserId, WereWolfRole>, ()>
where
    R: Rng,
{
    // Turn the Map of Roles into a list of all Roles
    let roles = {
        let mut tmp = Vec::new();
        for (role, count) in roles {
            for _ in 0..count {
                tmp.push(role.clone());
            }
        }
        tmp
    };

    // Update the Role-List to accomodate Roles that will turn into another one
    // while playing
    let mut roles = populate_nested_roles(roles, rng);

    if roles.len() != participants.len() {
        tracing::error!("Mismatched User-Role Count");
        return Err(());
    }

    let mut result = BTreeMap::new();
    for user in participants {
        let role_index = rng.gen::<usize>() % roles.len();
        let role = roles.remove(role_index);

        result.insert(user, role);
    }

    Ok(result)
}

/// This will distribute the given Roles to the Players
pub fn distribute_roles(
    participants: Vec<UserId>,
    roles: BTreeMap<WereWolfRole, usize>,
) -> Result<BTreeMap<UserId, WereWolfRole>, ()> {
    let mut rng = rand::thread_rng();

    distribute(participants, roles, &mut rng)
}

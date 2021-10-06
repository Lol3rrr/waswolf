use std::collections::BTreeMap;

use rand::Rng;
use serenity::model::id::UserId;

use super::{WereWolfRoleConfig, WereWolfRoleInstance};

#[derive(Debug)]
pub enum DistributeError {
    MismatchedCount {
        available_roles: usize,
        player_count: usize,
    },
    TooManyMaskedRoles {
        masking_roles: usize,
        normal_roles: usize,
    },
}

/// This will actually distribute the Roles among the Players
fn distribute<R>(
    mut participants: Vec<UserId>,
    roles: BTreeMap<WereWolfRoleConfig, usize>,
    rng: &mut R,
) -> Result<BTreeMap<UserId, WereWolfRoleInstance>, DistributeError>
where
    R: Rng,
{
    let mut nested_roles = {
        let mut tmp = Vec::new();
        for (role, count) in roles
            .iter()
            .filter(|(r, _)| r.masks_role())
            .map(|(r, c)| (r, *c))
        {
            for _ in 0..count {
                tmp.push(role.clone());
            }
        }
        tmp
    };

    let mut non_nested_roles = {
        let mut tmp = Vec::new();
        for (role, count) in roles
            .iter()
            .filter(|(r, _)| !r.masks_role())
            .map(|(r, c)| (r, *c))
        {
            for _ in 0..count {
                tmp.push(role.clone());
            }
        }
        tmp
    };

    if non_nested_roles.len() != participants.len() {
        return Err(DistributeError::MismatchedCount {
            available_roles: non_nested_roles.len(),
            player_count: participants.len(),
        });
    }

    if nested_roles.len() > non_nested_roles.len() {
        return Err(DistributeError::TooManyMaskedRoles {
            masking_roles: nested_roles.len(),
            normal_roles: non_nested_roles.len(),
        });
    }

    let mut result = BTreeMap::new();
    for nested_roles_remaining in nested_roles.len()..0 {
        let index = rng.gen_range(0..nested_roles_remaining);
        let nested_role = nested_roles.remove(index);

        let user = participants.pop().unwrap();
        let instance = nested_role.to_instance(&mut non_nested_roles, rng).unwrap();

        result.insert(user, instance);
    }
    for r_remaining in non_nested_roles.len()..0 {
        let index = rng.gen_range(0..r_remaining);
        let role = non_nested_roles.remove(index);

        let user = participants.pop().unwrap();
        let instance = role.to_instance(&mut non_nested_roles, rng).unwrap();

        result.insert(user, instance);
    }

    Ok(result)
}

/// This will distribute the given Roles to the Players
pub fn distribute_roles(
    participants: Vec<UserId>,
    roles: BTreeMap<WereWolfRoleConfig, usize>,
) -> Result<BTreeMap<UserId, WereWolfRoleInstance>, DistributeError> {
    let mut rng = rand::thread_rng();

    distribute(participants, roles, &mut rng)
}

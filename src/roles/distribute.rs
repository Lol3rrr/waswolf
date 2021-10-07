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

fn get_roles<'i, I, F>(roles: I, check: F) -> Vec<WereWolfRoleConfig>
where
    F: Fn(&WereWolfRoleConfig) -> bool,
    I: Iterator<Item = (&'i WereWolfRoleConfig, &'i usize)>,
{
    let mut result = Vec::new();

    let iter = roles.filter(|(r, _)| check(r)).map(|(r, c)| (r, *c));
    for (role, count) in iter {
        for _ in 0..count {
            result.push(role.clone());
        }
    }

    result
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
    let mut nested_roles = get_roles(roles.iter(), |r| r.masks_role());
    let mut non_nested_roles = get_roles(roles.iter(), |r| !r.masks_role());

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
    for nested_roles_remaining in (1..=nested_roles.len()).rev() {
        let index = rng.gen_range(0..nested_roles_remaining);
        let nested_role = nested_roles.remove(index);

        let user = participants.pop().unwrap();
        let instance = nested_role.to_instance(&mut non_nested_roles, rng).unwrap();

        result.insert(user, instance);
    }
    for r_remaining in (1..=non_nested_roles.len()).rev() {
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

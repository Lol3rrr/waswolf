use crate::Reactions;

use super::WereWolfRole;

const MAX_REACTIONS: usize = 17;

/// Checks if the given Page is the last Page for the Role selection
fn is_last_page(role_count: usize, page: usize) -> bool {
    if role_count == 0 {
        return true;
    }
    page >= (role_count - 1) / MAX_REACTIONS
}

/// Generates the list of reactions for the given List of Roles and the correct Page
pub fn reactions(roles: &[WereWolfRole], page: usize) -> Vec<Reactions> {
    let mut result = Vec::new();

    // If it is not the first Page, we first add the PreviousPage Reaction as all
    // pages need a "back" button
    if page > 0 {
        result.push(Reactions::PreviousPage);
    }

    // Add the correct Reactions for all the Roles
    for raw_index in 0..MAX_REACTIONS {
        let index = raw_index + page * MAX_REACTIONS;

        let role = match roles.get(index) {
            Some(r) => r,
            None => break,
        };
        result.push(Reactions::Custom(role.to_emoji().to_string()));
    }

    // If it is not the last page, we need to add a button to navigate to the next page
    if !is_last_page(roles.len(), page) {
        result.push(Reactions::NextPage);
    }

    result.push(Reactions::Confirm);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_last_page_true() {
        assert!(is_last_page(15, 0));
        assert!(is_last_page(17, 0));
        assert!(is_last_page(18, 1));
        assert!(is_last_page(34, 1));
    }
    #[test]
    fn is_last_page_false() {
        assert!(!is_last_page(18, 0));
        assert!(!is_last_page(35, 1));
    }

    #[test]
    fn empty_roles() {
        let roles = &[];
        let page = 0;

        let result = reactions(roles, page);
        let expected: Vec<Reactions> = vec![Reactions::Confirm];
        assert_eq!(expected, result);
    }

    #[test]
    fn first_page() {
        let roles = vec![WereWolfRole::Werwolf; 30];
        let page = 0;

        let result = reactions(&roles, page);
        let expected: Vec<Reactions> = {
            let mut tmp = vec![Reactions::Custom(WereWolfRole::Werwolf.to_emoji().to_string()); 17];
            tmp.push(Reactions::NextPage);
            tmp.push(Reactions::Confirm);
            tmp
        };
        assert_eq!(expected, result);
    }
    #[test]
    fn middle_page() {
        let roles = vec![WereWolfRole::Werwolf; 50];
        let page = 1;

        let result = reactions(&roles, page);
        let expected: Vec<Reactions> = {
            let mut tmp = vec![Reactions::PreviousPage];
            tmp.extend(vec![
                Reactions::Custom(
                    WereWolfRole::Werwolf.to_emoji().to_string()
                );
                17
            ]);
            tmp.push(Reactions::NextPage);
            tmp.push(Reactions::Confirm);
            tmp
        };
        assert_eq!(expected, result);
    }
    #[test]
    fn last_page() {
        let roles = vec![WereWolfRole::Werwolf; 17 * 3];
        let page = 2;

        let result = reactions(&roles, page);
        let expected: Vec<Reactions> = {
            let mut tmp = vec![Reactions::PreviousPage];
            tmp.extend(vec![
                Reactions::Custom(
                    WereWolfRole::Werwolf.to_emoji().to_string()
                );
                17
            ]);
            tmp.push(Reactions::Confirm);
            tmp
        };
        assert_eq!(expected, result);
    }
}

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Số dư ròng của một thành viên: dương = được nhận, âm = đang nợ.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBalance {
    pub user_id: Uuid,
    pub net_amount: i64, // positive = owed money, negative = owes money
}

/// Một giao dịch trả nợ được gợi ý để cân bằng nhóm.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementSuggestion {
    pub from_user_id: Uuid, // Person who owes money
    pub to_user_id: Uuid,   // Person who is owed money
    pub amount: i64,        // Amount to settle
}

/// Phần chia của một thành viên trong một expense (đầu vào engine).
#[derive(Debug, Clone)]
pub struct ShareForEngine {
    pub user_id: Uuid,
    pub amount: i64,
}

/// Một expense rút gọn (đầu vào engine).
#[derive(Debug, Clone)]
pub struct ExpenseForEngine {
    pub payer_id: Uuid,
    pub amount: i64,
    pub shares: Vec<ShareForEngine>,
}

/// Một settlement đã chốt (đầu vào engine).
#[derive(Debug, Clone)]
pub struct SettlementForEngine {
    pub sender_id: Uuid,
    pub receiver_id: Uuid,
    pub amount: i64,
}

pub struct DebtEngine;

impl DebtEngine {
    /// Optimized Greedy algorithm with Exact-Match Heuristic
    /// to minimize both transaction count and money fragmentation.
    fn simplify_debts_blocking(balances: &[UserBalance]) -> Vec<SettlementSuggestion> {
        let mut debtors: Vec<(Uuid, i64)> = Vec::with_capacity(balances.len());
        let mut creditors: Vec<(Uuid, i64)> = Vec::with_capacity(balances.len());

        for b in balances {
            if b.net_amount < 0 {
                debtors.push((b.user_id, -b.net_amount));
            } else if b.net_amount > 0 {
                creditors.push((b.user_id, b.net_amount));
            }
        }

        let mut suggestions = Vec::with_capacity(debtors.len() + creditors.len());

        // 1. Exact Match Optimization: Pair up identical debt & credit amounts directly
        let mut d_idx = 0;
        while d_idx < debtors.len() {
            let (d_id, d_amt) = debtors[d_idx];
            if let Some(c_idx) = creditors.iter().position(|&(_, c_amt)| c_amt == d_amt) {
                let (c_id, _) = creditors.swap_remove(c_idx);
                suggestions.push(SettlementSuggestion { from_user_id: d_id, to_user_id: c_id, amount: d_amt });
                debtors.swap_remove(d_idx);
            } else {
                d_idx += 1;
            }
        }

        // 2. Sort remaining participants descending by amount
        debtors.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));
        creditors.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));

        let mut debtor_idx = 0;
        let mut creditor_idx = 0;

        // 3. Greedy Two-Pointer Matching
        while debtor_idx < debtors.len() && creditor_idx < creditors.len() {
            let (debtor_id, debtor_amount) = debtors[debtor_idx];
            let (creditor_id, creditor_amount) = creditors[creditor_idx];

            let settlement_amount = debtor_amount.min(creditor_amount);

            if settlement_amount > 0 {
                suggestions.push(SettlementSuggestion {
                    from_user_id: debtor_id,
                    to_user_id: creditor_id,
                    amount: settlement_amount,
                });
            }

            debtors[debtor_idx].1 -= settlement_amount;
            creditors[creditor_idx].1 -= settlement_amount;

            if debtors[debtor_idx].1 == 0 {
                debtor_idx += 1;
            }
            if creditors[creditor_idx].1 == 0 {
                creditor_idx += 1;
            }
        }

        suggestions
    }

    /// Calculates net balance for every member in the group.
    /// Positive = is owed money, Negative = owes money.
    fn calculate_net_balances_blocking(
        user_ids: &[Uuid],
        expenses: &[ExpenseForEngine],
        settlements: &[SettlementForEngine],
    ) -> Vec<UserBalance> {
        let mut balance_map: HashMap<Uuid, i64> = HashMap::with_capacity(user_ids.len());
        for &id in user_ids {
            balance_map.insert(id, 0);
        }

        // 1. Process Expenses
        for expense in expenses {
            *balance_map.entry(expense.payer_id).or_insert(0) += expense.amount;
            for share in &expense.shares {
                *balance_map.entry(share.user_id).or_insert(0) -= share.amount;
            }
        }

        // 2. Process Settlements
        for settlement in settlements {
            *balance_map.entry(settlement.sender_id).or_insert(0) += settlement.amount;
            *balance_map.entry(settlement.receiver_id).or_insert(0) -= settlement.amount;
        }

        let mut result: Vec<UserBalance> =
            balance_map.into_iter().map(|(user_id, net_amount)| UserBalance { user_id, net_amount }).collect();

        result.sort_unstable_by_key(|a| a.user_id);
        result
    }

    /// Gợi ý các giao dịch trả nợ tối thiểu từ bảng số dư (greedy + exact-match).
    ///
    /// Để sync vì thuần tính toán trên memory, không I/O — caller async cứ gọi trực tiếp,
    /// không cần `spawn_blocking` với input cỡ nhóm chat.
    #[tracing::instrument(skip(balances), fields(n = balances.len()))]
    #[must_use]
    pub fn simplify_debts(balances: &[UserBalance]) -> Vec<SettlementSuggestion> {
        Self::simplify_debts_blocking(balances)
    }

    /// Tính số dư ròng từng thành viên từ expenses và settlements đã chốt.
    ///
    /// Để sync vì lý do như [`DebtEngine::simplify_debts`].
    #[tracing::instrument(skip(user_ids, expenses, settlements), fields(n = user_ids.len()))]
    #[must_use]
    pub fn calculate_net_balances(
        user_ids: &[Uuid],
        expenses: &[ExpenseForEngine],
        settlements: &[SettlementForEngine],
    ) -> Vec<UserBalance> {
        Self::calculate_net_balances_blocking(user_ids, expenses, settlements)
    }
}

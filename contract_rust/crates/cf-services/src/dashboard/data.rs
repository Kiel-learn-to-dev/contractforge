use cf_db::store::Store;
use chrono::{NaiveDate, NaiveDateTime};

use super::{
    actions::{self, ActionItem},
    breakdown::{self, ProductBreakdown, RecentActivity},
    expiring_page::{self, ExpiringContract},
    summary::{self, DashboardSummaryData},
    trends::{self, ExpiryForecastMonth, MonthlyStats, TopCustomer},
};

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardData {
    pub summary: DashboardSummaryData,
    pub expiring_30: Vec<ExpiringContract>,
    pub expiring_31_60: Vec<ExpiringContract>,
    pub by_product: Vec<ProductBreakdown>,
    pub recent_activity: Vec<RecentActivity>,
    pub monthly_stats: MonthlyStats,
    pub top_customers: Vec<TopCustomer>,
    pub monthly_expiry_forecast: Vec<ExpiryForecastMonth>,
    pub action_items: Vec<ActionItem>,
    pub today: NaiveDate,
}

/// Collect the service data needed by the dashboard from a single in-memory Store.
pub fn get_dashboard_data(
    store: &Store,
    today: NaiveDate,
    now_utc: NaiveDateTime,
) -> DashboardData {
    let expiring_30 = expiring_page::get_expiring_contracts(store, today, 30);
    let expiring_60 = expiring_page::get_expiring_contracts(store, today, 60);
    let expiring_31_60 = expiring_60
        .into_iter()
        .filter(|row| row.days_left > 30)
        .collect();
    DashboardData {
        summary: summary::get_summary(store, today),
        expiring_30,
        expiring_31_60,
        by_product: breakdown::contracts_by_product(store),
        recent_activity: breakdown::recent_activity(store),
        monthly_stats: trends::monthly_stats(store, today),
        top_customers: trends::top_customers(store, 5),
        monthly_expiry_forecast: trends::monthly_expiry_forecast(store, today, 12),
        action_items: actions::get_action_items(store, today, now_utc),
        today,
    }
}

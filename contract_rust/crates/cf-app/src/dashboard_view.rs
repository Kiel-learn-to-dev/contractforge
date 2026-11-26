//! Chuẩn bị dữ liệu đã định dạng cho màn hình dashboard (S01).

use std::{f64::consts::PI, time::Instant};

use cf_core::{fmt::fmt_currency, lifecycle::ContractStatus};
use cf_services::{
    Ctx,
    dashboard::{self, actions::ScreenTarget},
};
use chrono::Datelike;

use crate::nav::Screen;

const PIE_COLORS: [[u8; 3]; 7] = [
    [0, 63, 177],    // #003fb1 cobalt
    [22, 163, 74],   // #16a34a green
    [217, 119, 6],   // #d97706 amber
    [124, 58, 237],  // #7c3aed purple
    [13, 148, 136],  // #0d9488 teal
    [37, 99, 235],   // #2563eb blue
    [100, 116, 139], // #64748b slate
];

#[derive(Debug, Clone)]
pub struct Metric {
    pub label: String,
    pub value: String,
    pub route: i32,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct Action {
    pub message: String,
    pub level: String,
    pub route: i32,
    pub target: ScreenTarget,
}

#[derive(Debug, Clone)]
pub struct ForecastBar {
    pub label: String,
    pub count: usize,
    pub ratio: f32,
}

#[derive(Debug, Clone)]
pub struct StatusDist {
    pub label: String,
    pub count: usize,
    pub ratio: f32,
    pub rgb: [u8; 3],
    pub status_code: String,
}

#[derive(Debug, Clone)]
pub struct PieSlice {
    pub name: String,
    pub count: usize,
    pub value_text: String,
    pub rgb: [u8; 3],
    pub commands: String,
}

#[derive(Debug, Clone)]
pub struct ExpiringSummary {
    pub contract_id: i64,
    pub contract_number: String,
    pub customer_name: String,
    pub end_date: String,
    pub days_left: i64,
    pub urgency: &'static str,
}

#[derive(Debug, Clone)]
pub struct TopCustomerItem {
    pub id: i64,
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct ActivityItem {
    pub contract_id: i64,
    pub title: String,
    pub subtitle: String,
    pub when_text: String,
}

#[derive(Debug, Clone)]
pub struct DashboardView {
    pub today_label: String,
    pub metrics: Vec<Metric>,
    pub actions: Vec<Action>,
    pub forecast_bars: Vec<ForecastBar>,
    pub status_dist: Vec<StatusDist>,
    pub pie_items: Vec<PieSlice>,
    pub expiring_30: Vec<ExpiringSummary>,
    pub expiring_31_60: Vec<ExpiringSummary>,
    pub value_total_text: String,
    pub value_in_force_sub: String,
    pub value_paid_text: String,
    pub value_invoiced_text: String,
    pub value_unpaid_text: String,
    pub monthly_title: String,
    pub monthly_new_text: String,
    pub monthly_signed_text: String,
    pub monthly_expired_text: String,
    pub monthly_value_text: String,
    pub top_customers: Vec<TopCustomerItem>,
    pub recent_activities: Vec<ActivityItem>,
}

pub fn load(ctx: &Ctx) -> DashboardView {
    let started = Instant::now();
    let now_utc = ctx.clock().now_utc();
    let today = ctx.clock().now_local().date();
    let data = dashboard::data::get_dashboard_data(ctx.store(), today, now_utc);
    let summary = data.summary.summary;
    let contracts = Screen::Contracts.index();

    // Khớp 6 thẻ stat & màu sắc của ContractForge/templates/dashboard/index.html
    let metrics = vec![
        metric(
            "Đang cung cấp dịch vụ",
            summary.in_service.to_string(),
            contracts,
            [22, 163, 74],
        ),
        metric(
            "Đã ký (chờ hóa đơn)",
            summary.signed.to_string(),
            contracts,
            [13, 148, 136],
        ),
        metric(
            "Sắp hết hạn (≤30d)",
            summary.expiring_soon.to_string(),
            contracts,
            [217, 119, 6],
        ),
        metric(
            "Đã hết hạn",
            summary.expired.to_string(),
            contracts,
            [220, 38, 38],
        ),
        metric(
            "Khách hàng",
            summary.total_customers.to_string(),
            Screen::Customers.index(),
            [37, 99, 235],
        ),
        metric(
            "Đã từng sinh file",
            summary.generated.to_string(),
            contracts,
            [79, 70, 229],
        ),
    ];

    let actions = data
        .action_items
        .into_iter()
        .map(|item| Action {
            message: item.message,
            level: item.level.to_owned(),
            route: match &item.target {
                ScreenTarget::NewTemplate => Screen::Templates.index(),
                ScreenTarget::Expiring { .. } | ScreenTarget::ContractsByStatus { .. } => contracts,
            },
            target: item.target,
        })
        .collect();

    let max_bar = data
        .monthly_expiry_forecast
        .iter()
        .map(|m| m.count)
        .max()
        .unwrap_or(1)
        .max(1);
    let forecast_bars = data
        .monthly_expiry_forecast
        .into_iter()
        .map(|m| ForecastBar {
            label: m.label,
            count: m.count,
            ratio: (m.count as f32) / (max_bar as f32),
        })
        .collect();

    let total_all = summary.total_contracts.max(1) as f32;
    let dist_specs: [(&str, [u8; 3]); 7] = [
        ("Draft", [100, 116, 139]),
        ("Generated", [79, 70, 229]),
        ("Sent", [37, 99, 235]),
        ("Signed", [13, 148, 136]),
        ("Invoiced", [217, 119, 6]),
        ("PaidActive", [22, 163, 74]),
        ("Expired", [220, 38, 38]),
    ];
    let status_dist = dist_specs
        .into_iter()
        .filter_map(|(code, rgb)| {
            let count = data.summary.status_counts.get(code).copied().unwrap_or(0);
            if count == 0 {
                return None;
            }
            let label = code
                .parse::<ContractStatus>()
                .map(|s| s.label().to_owned())
                .unwrap_or_else(|_| code.to_owned());
            Some(StatusDist {
                label,
                count,
                ratio: (count as f32) / total_all,
                rgb,
                status_code: code.to_owned(),
            })
        })
        .collect();

    let pie_items = build_pie_slices(&data.by_product);

    let map_expiring =
        |rows: Vec<dashboard::expiring_page::ExpiringContract>| -> Vec<ExpiringSummary> {
            rows.into_iter()
                .take(6)
                .map(|r| ExpiringSummary {
                    contract_id: r.contract_id,
                    contract_number: r.contract_number,
                    customer_name: r.customer_name,
                    end_date: r.end_date.format("%d/%m/%Y").to_string(),
                    days_left: r.days_left,
                    urgency: r.urgency,
                })
                .collect()
        };

    let ms = data.monthly_stats;
    let delta_fmt = |d: isize| -> String {
        if d > 0 {
            format!(" (+{d})")
        } else if d < 0 {
            format!(" ({d})")
        } else {
            String::new()
        }
    };

    let top_customers = data
        .top_customers
        .into_iter()
        .map(|c| TopCustomerItem {
            id: c.id,
            name: c.name,
            count: c.count,
        })
        .collect();

    let recent_activities = data
        .recent_activity
        .into_iter()
        .map(|a| ActivityItem {
            contract_id: a.contract_id,
            title: a.contract_number,
            subtitle: format!("{} — {}", a.customer_name, a.description),
            when_text: a.occurred_at.format("%d/%m %H:%M").to_string(),
        })
        .collect();

    let weekday = match today.weekday() {
        chrono::Weekday::Mon => "Th 2",
        chrono::Weekday::Tue => "Th 3",
        chrono::Weekday::Wed => "Th 4",
        chrono::Weekday::Thu => "Th 5",
        chrono::Weekday::Fri => "Th 6",
        chrono::Weekday::Sat => "Th 7",
        chrono::Weekday::Sun => "CN",
    };

    let view = DashboardView {
        today_label: format!("{weekday}, {}", today.format("%d/%m/%Y")),
        metrics,
        actions,
        forecast_bars,
        status_dist,
        pie_items,
        expiring_30: map_expiring(data.expiring_30),
        expiring_31_60: map_expiring(data.expiring_31_60),
        value_total_text: fmt_currency(Some(summary.total_value_active)),
        value_in_force_sub: format!("{} HĐ trong vòng đời hiệu lực", summary.in_force),
        value_paid_text: fmt_currency(Some(summary.value_paid)),
        value_invoiced_text: fmt_currency(Some(summary.value_invoiced)),
        value_unpaid_text: fmt_currency(Some(summary.value_unpaid)),
        monthly_title: ms.month_label,
        monthly_new_text: format!("{} HĐ mới tạo{}", ms.new_contracts, delta_fmt(ms.vs_new)),
        monthly_signed_text: format!("{} HĐ đã ký{}", ms.signed, delta_fmt(ms.vs_signed)),
        monthly_expired_text: format!("{} HĐ hết hạn tháng này", ms.expired_count),
        monthly_value_text: format!("Giá trị mới: {}", fmt_currency(Some(ms.new_value))),
        top_customers,
        recent_activities,
    };
    tracing::info!("tải dashboard trong {} ms", started.elapsed().as_millis());
    view
}

fn build_pie_slices(items: &[dashboard::breakdown::ProductBreakdown]) -> Vec<PieSlice> {
    let total_count: usize = items.iter().map(|i| i.count).sum();
    if total_count == 0 {
        return Vec::new();
    }
    let mut slices = Vec::with_capacity(items.len());
    let mut start_angle = -PI / 2.0;
    for (idx, item) in items.iter().enumerate() {
        let rgb = PIE_COLORS[idx % PIE_COLORS.len()];
        let fraction = (item.count as f64) / (total_count as f64);
        let commands = if item.count == total_count {
            "M 50 10 A 40 40 0 1 1 50 90 A 40 40 0 1 1 50 10 Z".to_owned()
        } else {
            let sweep = fraction * 2.0 * PI;
            let end_angle = start_angle + sweep;
            let x1 = 50.0 + 40.0 * start_angle.cos();
            let y1 = 50.0 + 40.0 * start_angle.sin();
            let x2 = 50.0 + 40.0 * end_angle.cos();
            let y2 = 50.0 + 40.0 * end_angle.sin();
            let large_arc = if sweep > PI { 1 } else { 0 };
            start_angle = end_angle;
            format!("M 50 50 L {x1:.2} {y1:.2} A 40 40 0 {large_arc} 1 {x2:.2} {y2:.2} Z")
        };
        slices.push(PieSlice {
            name: item.name.clone(),
            count: item.count,
            value_text: fmt_currency(Some(item.total_value)),
            rgb,
            commands,
        });
    }
    slices
}

fn metric(label: &str, value: String, route: i32, rgb: [u8; 3]) -> Metric {
    Metric {
        label: label.into(),
        value,
        route,
        rgb,
    }
}

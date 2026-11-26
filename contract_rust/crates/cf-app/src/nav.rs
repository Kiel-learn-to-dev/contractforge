//! Điều hướng màn hình: enum `Screen` và ngăn xếp Quay lại tối đa 20 mục (05_GIAO_DIEN §2).

/// Chín mục của thanh bên, theo đúng thứ tự hiển thị.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Customers,
    Products,
    Templates,
    Contracts,
    Generate,
    Batch,
    Quotations,
    Settings,
}

impl Screen {
    pub const ALL: [Screen; 9] = [
        Screen::Dashboard,
        Screen::Customers,
        Screen::Products,
        Screen::Templates,
        Screen::Contracts,
        Screen::Generate,
        Screen::Batch,
        Screen::Quotations,
        Screen::Settings,
    ];

    /// Tiêu đề hiện trên thanh bên và trên đầu màn hình.
    pub fn title(self) -> &'static str {
        match self {
            Screen::Dashboard => "Dashboard",
            Screen::Customers => "Khách hàng",
            Screen::Products => "Sản phẩm",
            Screen::Templates => "Mẫu hợp đồng",
            Screen::Contracts => "DS hợp đồng",
            Screen::Generate => "Sinh hợp đồng",
            Screen::Batch => "Sinh hàng loạt",
            Screen::Quotations => "Báo giá",
            Screen::Settings => "Cài đặt",
        }
    }

    /// Vị trí trong thanh bên (0-based).
    pub fn index(self) -> i32 {
        Self::ALL
            .iter()
            .position(|screen| *screen == self)
            .map_or(0, |position| i32::try_from(position).unwrap_or(0))
    }

    /// Màn hình ứng với vị trí trên thanh bên; ngoài phạm vi thì None.
    pub fn from_index(index: i32) -> Option<Screen> {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// Số mục tối đa giữ trong ngăn xếp Quay lại.
pub const HISTORY_LIMIT: usize = 20;

/// Màn hình hiện tại và lịch sử để Quay lại.
#[derive(Debug)]
pub struct Nav {
    current: Screen,
    history: Vec<Screen>,
}

impl Default for Nav {
    fn default() -> Self {
        Self {
            current: Screen::Dashboard,
            history: Vec::new(),
        }
    }
}

impl Nav {
    pub fn current(&self) -> Screen {
        self.current
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    /// Chuyển sang màn hình khác. Chọn đúng màn hình đang mở thì không làm gì.
    pub fn navigate(&mut self, screen: Screen) {
        if screen == self.current {
            return;
        }
        self.history.push(self.current);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.current = screen;
    }

    /// Quay lại màn hình trước. Trả về màn hình mới, hoặc None nếu không còn lịch sử.
    pub fn back(&mut self) -> Option<Screen> {
        let previous = self.history.pop()?;
        self.current = previous;
        Some(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::{HISTORY_LIMIT, Nav, Screen};

    #[test]
    fn navigate_then_back_returns_to_previous_screen() {
        let mut nav = Nav::default();
        nav.navigate(Screen::Customers);
        nav.navigate(Screen::Products);
        assert_eq!(nav.current(), Screen::Products);
        assert_eq!(nav.back(), Some(Screen::Customers));
        assert_eq!(nav.back(), Some(Screen::Dashboard));
        assert_eq!(nav.back(), None);
        assert!(!nav.can_go_back());
    }

    #[test]
    fn same_screen_is_not_pushed() {
        let mut nav = Nav::default();
        nav.navigate(Screen::Dashboard);
        assert!(!nav.can_go_back());
    }

    #[test]
    fn history_is_capped_at_limit() {
        let mut nav = Nav::default();
        for index in 0..(HISTORY_LIMIT + 5) {
            let screen = Screen::ALL[index % Screen::ALL.len()];
            nav.navigate(screen);
        }
        let mut steps = 0;
        while nav.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_LIMIT);
    }

    #[test]
    fn index_round_trips() {
        for screen in Screen::ALL {
            assert_eq!(Screen::from_index(screen.index()), Some(screen));
        }
        assert_eq!(Screen::from_index(9), None);
        assert_eq!(Screen::from_index(-1), None);
    }
}

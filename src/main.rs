use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use scraper::{Html, Selector};

const URL: &'static str = "https://www.cskh.evnspc.vn/TraCuu/GetThongTinLichNgungGiamCungCapDien?maKH=PB17020000767&ChucNang=MaKhachHang";

const HTML1: &'static str = r#"
    <div class="notification">
  <div>
    <p class="ttl">
      <span class="red">Thông báo lịch ngừng giảm cung cấp điện</span><br />
      <span><b>Khách hàng:</b> L&#xFD; Th&#x1ECB; Th&#xF4;i</span>
      <br />
      <span
        ><b>Địa chỉ:</b> &#x1EA4;p Khu 1, X&#xE3; Nhu Gia, Th&#xE0;nh ph&#x1ED1;
        C&#x1EA7;n Th&#x1A1;</span
      >
      <br />
    </p>

    <div class="entry">
      <span class="code"><b>MÃ LỊCH:</b> 756722</span>
      <br />
      <span class="time">
        <b>THỜI GIAN:</b>
        <span style="white-space: nowrap">
          t&#x1EEB; 07:00:00 ng&#xE0;y 11/10/2026 &#x111;&#x1EBF;n 12:00:00
          ng&#xE0;y 11/10/2026
        </span>
      </span>
      <br />
      <span class="cause">
        <b style="white-space: nowrap; display: inline-block"
          >LÝ DO NGỪNG CUNG CẤP ĐIỆN:</b
        >
        <span
          >B&#x1EA3;o tr&#xEC;, s&#x1EED;a ch&#x1EEF;a l&#x1B0;&#x1EDB;i
          &#x111;i&#x1EC7;n</span
        >
      </span>
      <br />
    </div>
    <br />
  </div>
</div>
    "#;

const HTML2: &'static str = r#"
<div class="notification">
  <div>
    <p class="ttl">
      Hiện tại khách hàng<br />
      <small class="red">không có lịch ngừng giảm cung cấp điện</small>
    </p>
    <p class="time">
      <span>từ ngày 09-10-2026</span>
      <span>đến ngày 13-10-2026</span>
    </p>
  </div>
</div>"#;
#[derive(Debug)]
struct Schedule {
    code: String,
    from_at: DateTime<Utc>,
    to_at: DateTime<Utc>,

    cause: String,
}
#[derive(Debug)]
struct Customer {
    name: String,
    address: String,
    schedules: Vec<Schedule>,
}

fn parse_time_range(input: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let parts: Vec<&str> = input.split(" đến ").collect();

    if parts.len() != 2 {
        return Err("Khoảng thời gian không hợp lệ".to_string());
    }

    let parse_datetime = |s: &str| -> Result<DateTime<Utc>, String> {
        let s = s.trim().strip_prefix("từ ").unwrap_or(s.trim());

        let (time, date) = s.split_once(" ngày ").ok_or("Không tìm thấy ngày")?;

        let datetime_str = format!("{} {}", date.trim(), time.trim());

        let naive = NaiveDateTime::parse_from_str(&datetime_str, "%d/%m/%Y %H:%M:%S")
            .map_err(|e| e.to_string())?;

        let offset = FixedOffset::east_opt(7 * 3600).unwrap();

        Ok(naive
            .and_local_timezone(offset)
            .single()
            .ok_or("Không xác định được múi giờ")?
            .with_timezone(&Utc))
    };

    let from_at = parse_datetime(parts[0])?;
    let to_at = parse_datetime(parts[1])?;

    if from_at > to_at {
        return Err("Thời gian bắt đầu lớn hơn thời gian kết thúc".to_string());
    }

    Ok((from_at, to_at))
}

fn parser_html(html: &str) -> Customer {
    let document = Html::parse_fragment(html);
    let header_selector = Selector::parse("p.ttl > span:has(> b)").unwrap();

    let mut cus = Customer {
        name: String::new(),
        address: String::new(),
        schedules: vec![],
    };

    for span in document.select(&header_selector) {
        let label = span
            .select(&Selector::parse("b").unwrap())
            .next()
            .map(|b| b.text().collect::<String>())
            .unwrap_or_default();

        let value = span
            .text()
            .collect::<String>()
            .replace(&label, "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        match label.trim() {
            "Khách hàng:" => cus.name = value,
            "Địa chỉ:" => cus.address = value,
            _ => {}
        }
    }

    let list_selector = Selector::parse(".entry").unwrap();

    for entry in document.select(&list_selector) {
        let span_selector = Selector::parse("span:has(> b)").unwrap();
        let mut schedule = Schedule {
            code: String::new(),
            // time: String::new(),
            from_at: Utc::now(),
            to_at: Utc::now(),
            cause: String::new(),
        };
        for span in entry.select(&span_selector) {
            let label = span
                .select(&Selector::parse("b").unwrap())
                .next()
                .map(|b| b.text().collect::<String>())
                .unwrap_or_default();

            let value = span
                .text()
                .collect::<String>()
                .replace(&label, "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();

            match label.trim() {
                "MÃ LỊCH:" => schedule.code = value,
                "THỜI GIAN:" => {
                    // schedule.time = value,
                    let (from_at, to_at) = parse_time_range(&value).unwrap();
                    schedule.from_at = from_at;
                    schedule.to_at = to_at;
                }
                "LÝ DO NGỪNG CUNG CẤP ĐIỆN:" => schedule.cause = value,
                _ => {}
            }
        }
        cus.schedules.push(schedule);
    }

    cus
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let body = reqwest::get(URL).await?.text().await?; // lấy HTML dạng String

    let cus = parser_html(&body);
    println!("{:#?}", cus);
    Ok(())
}

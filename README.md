# Vạn Cân: Auto Fishing

Game câu cá RPG màn hình dọc, chơi một tay, với **Auto Fishing là bộ điều khiển chiến thuật
chạy trên mô phỏng** — không phải nút bấm nhận thưởng.

Viết bằng **Java + libGDX**, build ra **H5 (web), Android và iOS** từ một codebase chung.

> Dự án lấy cảm hứng từ các cơ chế được mô tả công khai của thể loại (xem `docs/REFERENCE.md`).
> Toàn bộ IP — tên loài, nhân vật, kỹ năng, art, số liệu cân bằng — là nguyên gốc.

---

## Trạng thái hiện tại

| Hạng mục | Trạng thái |
|---|---|
| Lõi mô phỏng câu cá (deterministic, fixed-step) | ✅ Hoàn chỉnh, có test |
| Auto Fishing AI — 5 chiến thuật | ✅ Hoàn chỉnh, có test cân bằng |
| Hành vi cá — 6 archetype, 6 pha | ✅ Hoàn chỉnh |
| Nội dung: 32 loài, 6 ngư trường, 37 ngư cụ, 8 cần thủ, 6 kỹ năng | ✅ Có validator |
| Tiến trình: cấp, tiềm năng, ngư cụ, đội hình, đồ giám | ✅ Hoàn chỉnh |
| Kinh tế + sổ cái + chống nhận thưởng trùng | ✅ Hoàn chỉnh |
| Câu ngoại tuyến (idle) | ✅ Mô hình kỳ vọng, có test |
| UI dọc — 5 màn hình | ✅ Chạy được, đã kiểm bằng ảnh chụp |
| Build H5 / Android / iOS | ✅ Cấu hình đầy đủ (mức kiểm chứng: xem `docs/BUILDING.md`) |
| Bang hội, boss thế giới, giải đấu, live-ops | ❌ Chưa làm — xem `docs/GDD_COVERAGE.md` |
| Server thẩm quyền, chống gian lận thực thi | ❌ Chưa làm — client đã tách sẵn seam, xem `docs/ARCHITECTURE.md` |
| Âm thanh, hiệu ứng, art thành phẩm | ❌ Art hiện sinh procedural, xem `docs/ASSETS.md` |

---

## Chạy thử nhanh

```bash
./gradlew :lwjgl3:run          # bản desktop dùng để phát triển
./gradlew :core:test           # 29 test: xác định tính deterministic, cân bằng, nội dung, font
./gradlew :core:balanceReport  # bảng tỉ lệ bắt cá / vàng-mỗi-phút cho mọi zone và chiến thuật
./gradlew :html:dist           # bản web tĩnh -> html/build/dist
```

Chi tiết từng nền tảng: **[docs/BUILDING.md](docs/BUILDING.md)**.

---

## Vì sao libGDX chứ không phải Unity

GDD đề xuất Unity + C#. Yêu cầu của dự án là **Java** với client **H5 + Android + iOS**.
libGDX là lựa chọn Java duy nhất phủ đủ cả ba: GWT transpile sang JavaScript cho web,
backend Android gốc, và RoboVM biên dịch AOT cho iOS — từ cùng một module `core`.

Ràng buộc đáng kể nhất đến từ GWT: **không reflection, không `java.util.stream`, không
`String.format`, không thư viện native**. Đó là lý do bộ sinh số ngẫu nhiên được viết tay,
định dạng save được ghi thủ công, và font là bitmap dựng sẵn thay vì FreeType.

---

## Kiến trúc rút gọn

```
core/     Java thuần + libGDX. Không phụ thuộc nền tảng.
  sim/      Mô phỏng câu cá — deterministic, fixed-step, KHÔNG dùng libGDX.
  auto/     Bộ điều khiển Auto Fishing (đọc state, phát ra hành động).
  content/  Nạp và kiểm tra bảng dữ liệu JSON.
  meta/     Tiến trình, kho đồ, kinh tế, ngoại tuyến, lưu game.
  ui/       Bộ widget immediate-mode + art procedural.
  screen/   Năm màn hình dọc.
lwjgl3/   Launcher desktop (phát triển) + harness chụp màn hình.
html/     Build GWT cho web.
android/  Launcher Android.
ios/      Launcher RoboVM.
assets/   Bảng dữ liệu JSON + font bitmap.
tools/    Sinh font, sinh icon, sinh dữ liệu loài cá.
```

`core/sim` **không tham chiếu libGDX**. Cùng đoạn code đó chạy trong client, trong test,
trong harness cân bằng, và (không cần sửa) trong một game server JVM để đối soát kết quả.
Đó là nền tảng cho thiết kế server-authoritative ở GDD §19.

Đọc thêm: **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** · **[docs/BALANCE.md](docs/BALANCE.md)**

---

## Vòng lặp cốt lõi

```
Chọn ngư trường → cấu hình build → thả câu → cá cắn → đóng lưỡi
  → mô phỏng vật lý (lực căng / độ bền dây / khoảng cách / thể lực cá)
  → bắt được hoặc mất cá → vàng + KN + đồ giám → nâng cấp → mở zone khó hơn
```

Auto đọc đúng những chỉ số HUD hiển thị và phát ra đúng loại hành động mà ngón tay người
chơi phát ra. Vì vậy Auto **không thể** kiếm nhiều hơn năng lực thật của build — đúng theo
pillar "Auto ≠ idle-only".

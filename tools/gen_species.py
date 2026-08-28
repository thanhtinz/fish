#!/usr/bin/env python3
"""Generates assets/data/species.json from the tier curves validated by the balance harness.

The per-species numbers are derived rather than hand-written so that a rebalance means editing
one curve here instead of reconciling 30 stat blocks by hand. Run it after changing GEAR_LINE or
the derivation below, then re-run `gradle :core:test` to confirm the catch-rate bands still hold.
"""
import json, collections, os

# Line strength / raw pull of the gear tier each zone is designed around (docs/BALANCE.md).
GEAR_LINE = [60, 85, 120, 165, 225, 300]
GEAR_DPS  = [16, 25, 37, 52, 70, 92]   # rodPower + reelPower per tier

def species(sid, name, tier, arch, rarity, mid, *, speed_mul=1.0, value_mul=1.0,
            escape=0.06, desc=""):
    line, dps = GEAR_LINE[tier], GEAR_DPS[tier] * 0.65
    power = 0.297 * line
    stam  = dps * 1.05 * 7.0
    hp    = dps * 1.10 * 13.0
    return collections.OrderedDict([
        ("id", sid), ("name", name), ("rarity", rarity), ("archetype", arch),
        ("minWeight", round(mid*0.40, 2)), ("maxWeight", round(mid*2.20, 2)),
        ("weightBias", 2.4),
        ("baseHp", round(hp, 1)),           ("hpPerKg", round(hp/mid*0.55, 2)),
        ("baseStamina", round(stam, 1)),    ("staminaPerKg", round(stam/mid*0.55, 2)),
        ("basePower", round(power*0.55, 2)),("powerPerKg", round(power*0.45/mid, 3)),
        ("baseSpeed", round((2.2 + tier*0.35) * speed_mul, 2)), ("speedPerKg", 0.02),
        ("escapeRate", escape), ("hookDifficulty", round(1 + tier*0.035, 3)),
        ("depth", round(0.5 + tier*0.6, 1)),
        ("baseValue", int(round(8 * (1.85 ** tier) * value_mul))),
        ("baseXp", int(round(5 * (1.7 ** tier)))),
        ("description", desc),
    ])

S = [
    species("ca_ro",     "Cá Rô Đồng",   0, "RUNNER",     "COMMON",   0.35, desc="Nhỏ, nhanh, quẫy liên tục ở mép cỏ."),
    species("ca_diec",   "Cá Diếc",      0, "ERRATIC",    "COMMON",   0.5,  desc="Đổi hướng thất thường, hay làm hụt mồi."),
    species("ca_tram_co","Cá Trắm Cỏ",   0, "POWER_TANK", "UNCOMMON", 1.6,  desc="Chậm mà lì, kéo dài dai dẳng."),
    species("ca_qua",    "Cá Quả",       0, "TRICKSTER",  "RARE",     2.2,  escape=0.09, desc="Giả chết rồi bứt phá bất ngờ."),

    species("ca_chep",   "Cá Chép",      1, "POWER_TANK", "COMMON",   2.5,  desc="Trụ cột của mọi khúc sông."),
    species("ca_mai",    "Cá Mại",       1, "RUNNER",     "COMMON",   1.2,  speed_mul=1.15, desc="Bơi thành đàn, tăng tốc rất gắt."),
    species("ca_lang",   "Cá Lăng",      1, "ERRATIC",    "UNCOMMON", 4.0,  desc="Đâm ngang dòng, tạo đỉnh lực căng."),
    species("ca_chien",  "Cá Chiên",     1, "DIVER",      "RARE",     6.0,  desc="Ẩn dưới đá ngầm, chuyên lặn sâu."),
    species("ca_anh_vu", "Cá Anh Vũ",    1, "TRICKSTER",  "EPIC",     5.0,  value_mul=1.4, escape=0.10, desc="Loài quý, cực khó đọc nhịp."),

    species("ca_vuoc",   "Cá Vược",      2, "RUNNER",     "COMMON",   5.0,  desc="Săn mồi ven bờ, bứt tốc mạnh."),
    species("ca_hong",   "Cá Hồng Biển", 2, "ERRATIC",    "UNCOMMON", 7.0,  desc="Quẫy loạn khi thấy bóng thuyền."),
    species("ca_thu",    "Cá Thu",       2, "RUNNER",     "UNCOMMON", 9.0,  speed_mul=1.25, desc="Tốc độ cao, dễ kéo hết dây."),
    species("ca_mu",     "Cá Mú Nghệ",   2, "POWER_TANK", "RARE",     14.0, desc="Chui hốc đá, ghì lực khủng khiếp."),
    species("ca_nham",   "Cá Nhám Bão",  2, "DIVER",      "EPIC",     18.0, value_mul=1.3, desc="Xuất hiện khi biển động."),

    species("ca_tra_dau","Cá Tra Dầu",   3, "POWER_TANK", "COMMON",   20.0, desc="Khối lượng lớn, sức bền cao."),
    species("ca_duoi",   "Cá Đuối Vực",  3, "DIVER",      "UNCOMMON", 28.0, desc="Ép xuống đáy, phạt khoảng cách nặng."),
    species("ca_ngu",    "Cá Ngừ Đại Dương", 3, "RUNNER", "RARE",     35.0, speed_mul=1.2, desc="Chạy đường dài không mệt."),
    species("ca_co",     "Cá Cờ Kiếm",   3, "ERRATIC",    "EPIC",     42.0, value_mul=1.35, desc="Đổi pha đột ngột, phá nhịp Auto."),
    species("ca_ho",     "Cá Hô Vàng",   3, "TRICKSTER",  "LEGENDARY",30.0, value_mul=2.0, escape=0.11, desc="Truyền thuyết sông sâu, cực kỳ ranh mãnh."),

    species("ca_map_xam","Cá Mập Xám",   4, "POWER_TANK", "UNCOMMON", 55.0, desc="Cỗ máy ghì dây thuần túy."),
    species("ca_mo_neo", "Cá Mỏ Neo",    4, "DIVER",      "RARE",     70.0, desc="Neo mình xuống vực, gần như bất động."),
    species("ca_bac_tuyet","Cá Bạc Tuyết",4,"RUNNER",     "EPIC",     48.0, speed_mul=1.3, desc="Vảy trắng, lao đi như tên bắn."),
    species("giao_long", "Giao Long Con",4, "TRICKSTER",  "LEGENDARY",65.0, value_mul=2.2, escape=0.12, desc="Hậu duệ thủy tộc, đọc được ý người câu."),
    species("ca_ngoc_lan","Cá Ngọc Lan", 4, "ERRATIC",    "MYTHIC",   80.0, value_mul=3.0, desc="Chỉ nổi lên vào đêm không trăng."),

    species("ca_thien_ngu","Cá Thiên Ngư",5,"RUNNER",     "RARE",     120.0, desc="Bơi giữa tầng mây và nước."),
    species("ca_kinh",   "Cá Kình Cổ",   5, "POWER_TANK", "EPIC",     180.0, desc="Sinh vật thời hồng hoang."),
    species("ca_van_lan","Cá Vân Lân",   5, "DIVER",      "LEGENDARY",150.0, value_mul=2.4, desc="Vảy phản chiếu như gương."),
    species("ca_tinh_hai","Cá Tinh Hải", 5, "TRICKSTER",  "MYTHIC",   160.0, value_mul=3.2, escape=0.13, desc="Ảo ảnh của biển sao."),
    species("an_ha_ngu", "Ẩn Hà Ngư",    5, "ERRATIC",    "SECRET",   140.0, value_mul=6.0, desc="Chỉ ghi nhận vài lần trong lịch sử ngư phủ."),

    # Bosses: scripted phase rotation (GDD 7) and deliberately slower than runners, so the
    # fight is a tension duel rather than a spool race.
    species("boss_thuy_qui","Thủy Quỷ Đầm",1,"BOSS","EPIC",      12.0, speed_mul=0.55, value_mul=2.5, desc="Boss vùng nước ngọt."),
    species("boss_hai_xa", "Hải Xà Bão", 3, "BOSS", "LEGENDARY",  60.0, speed_mul=0.5,  value_mul=3.5, desc="Boss biển động, nhiều pha."),
    species("boss_long_ngu","Long Ngư Vương",5,"BOSS","MYTHIC",  260.0, speed_mul=0.45, value_mul=6.0, desc="Boss thế giới, cần cả bang hội."),
]

ids = [s["id"] for s in S]
assert len(ids) == len(set(ids)), "duplicate species id"
os.makedirs("assets/data", exist_ok=True)
with open("assets/data/species.json", "w", encoding="utf-8") as f:
    json.dump({"species": S}, f, ensure_ascii=False, indent=2)
print("wrote %d species" % len(S))

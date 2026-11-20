# KeyState-RS (`rust-overlay/`)

**KeyState-RS**의 Rust 소스 코드 디렉토리입니다. 전체 소개, 기능, 단축키 및 게임 호환성 가이드는 루트 [README.md](../README.md)를 참고해 주세요.

## 빠른 빌드 및 실행

```powershell
cargo build --release
.\target\release\keystate-rs.exe
```

## 핵심 기술 스택
- **오버레이**: `Direct3D 11` + `Direct2D 1.1` + `DirectWrite` + `DirectComposition` (GPU 직접 합성, 유휴 0fps 절전, Device Lost 자동 복구, Per-Monitor DPI V2)
- **설정 UI**: `eframe` / `egui` (`glow` 백엔드, 시스템 맑은 고딕 자동 로드, 반응형 도킹 패널 + 비주얼 키 캔버스 에디터)
- **입력 엔진**: `Raw Input (RIDEV_INPUTSINK)` / `WH_KEYBOARD_LL` + `WH_MOUSE_LL` / `GetAsyncKeyState` 500Hz 폴링 (Lock-free 핫패스, 원클릭 관리자 권한 재실행)

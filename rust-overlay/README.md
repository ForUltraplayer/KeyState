# KeyState-RS

기존 C++ KeyState V1.5의 **저지연 글로벌 입력 감지**와 **Smoothstep 글로우·히스토리 트랙 애니메이션** 감각은 그대로 계승하면서, **Direct2D + DirectComposition GPU 렌더링**과 **비주얼 키보드 캔버스 에디터(`egui`)**로 처음부터 Rust로 재작성한 경량 Windows 키보드/마우스 오버레이입니다.

## 주요 개선점
- **GPU 네이티브 투명 오버레이**: GDI+ CPU 비트맵 루프를 제거하고 `Direct3D11 + Direct2D 1.1 + DirectWrite + DirectComposition` 스왑체인 직접 합성으로 전환. 유휴 시 이벤트 기반 절전(CPU ~0%).
- **전용 입력 훅 스레드**: `WH_KEYBOARD_LL` + `WH_MOUSE_LL`(좌/중/우 + 사이드버튼 MB4/MB5)을 UI/렌더와 분리된 스레드에서 구동. IME(`VK_PROCESSKEY=229`) 및 자동 반복 입력 필터링 유지.
- **비주얼 키보드 배치 에디터**:
  - 캔버스에서 키 **드래그 이동 / 우하단 핸들 리사이즈 / 그리드 스냅 / Shift+클릭 다중 선택 정렬**
  - **연속 키보드 추가 모드** & 마우스 버튼 원클릭 추가
  - 6종 기본 레이아웃 프리셋 (`WASD + Space`, `FPS`, `리듬 4K`, `리듬 7K`, `방향키+ZX`, `AOS QWER`)
  - **자유 캔버스 모드** & **자동 가로 1줄 모드** 즉시 전환
- **게임별 프로필 저장/전환** (`profiles/*.json`)
- **16종 테마 프리셋 & 실시간 테마 편집**
- **실시간 KPS 차트 별도 GPU 오버레이** (꺾은선 / 산점도 / 막대 + 그라데이션 + 하단 자동 스냅)
- **시스템 트레이 & 글로벌 단축키** (`Ctrl+Shift+K/M/N/P/H/U`) 및 KPS JSON 녹화 (`records/*.json`)

## 빌드 및 실행

```powershell
cd rust-overlay
cargo build --release
.\target\release\keystate-rs.exe
```

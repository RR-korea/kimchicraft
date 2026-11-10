# KimchiCraft (김치크래프트) 🥢

<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/ai.storyteller.wordcraft.png" width="128" height="128" alt="KimchiCraft icon">
</p>

<p align="center">
  <b>대한민국 공공 및 비즈니스 환경에 최적화된 Pure Rust 오픈소스 차세대 워드프로세서</b><br>
  <i>(A fork of WordCraft with Native HWP/HWPX support & Dual HWP/Word UI Mode)</i>
</p>

<p align="center">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-2b47b5?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on macOS, Windows, Linux, BSD and Web" src="https://img.shields.io/badge/runs%20on-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux%20%C2%B7%20BSD%20%C2%B7%20Web-3b5bdb?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-2b47b5?style=flat-square">
  <img alt="Dual UI: Word & HWP" src="https://img.shields.io/badge/UI%20Mode-Word%20%26%20HWP-ff6b6b?style=flat-square">
  <img alt="Native HWP & HWPX Support" src="https://img.shields.io/badge/Format-HWP%20%C2%B7%20HWPX%20%C2%B7%20DOCX%20%C2%B7%20ODT-20c997?style=flat-square">
</p>

---

## 🌟 프로젝트 개요 (Overview)

**KimchiCraft**는 고성능 Pure Rust 워드프로세서인 [WordCraft](https://github.com/storytold/wordcraft)를 기반으로 포크되어, 한국의 표준 문서 포맷인 **HWP(5.x 바이너리) 및 HWPX(KS X 6101 표준) 파일의 네이티브 처리**와 한국 사용자들에게 익숙한 **아래아한글(HWP) 스타일 UI 및 단축키 시스템**을 결합한 차세대 크로스플랫폼 문서 에디터입니다.

대한민국 공공기관, 학교, 기업에서 널리 쓰이는 한글 문서를 외부 프로그램이나 유료 상용 오피스 없이도 Windows, macOS, Linux 및 웹 브라우저(WebAssembly)에서 빠르고 완벽하게 열고 편집할 수 있는 환경을 제공합니다.

---

## 🎯 핵심 기능 및 특장점 (Key Features)

### 1. Pure Rust 네이티브 HWPX 파서 (`wordcraft-formats::hwpx`)
- **무의존성 오픈 표준 처리:** 오픈소스 AI 공공문서 처리 엔진인 [kordoc](https://github.com/chrisryugj/kordoc)의 설계 원리를 인용하여, 외부 Node.js나 Python 런타임 없이 **순수 Rust(zip + quick-xml)**로 HWPX(KS X 6101 OWPML 표준)를 직접 파싱합니다.
- **HWP 단위 정밀 매핑:** $1\text{ pt} = 100\text{ HWPU}$ (HWP Unit) 공식에 따라 폰트 크기, 문단 여백, 표 크기를 소수점 단위까지 정확하게 레이아웃 엔진과 동기화합니다.
- **스타일 및 서식 완전 복원:** `Contents/header.xml`에 정의된 글꼴, 글자 모양(`charPr`), 문단 모양(`paraPr`), 테두리 및 배경 채우기(`borderFill`)를 완벽하게 해석하여 문서 AST에 반영합니다.
- **복합 표(Table) 구조 복원:** `Contents/section*.xml`의 셀 가로/세로 병합(`colSpan`, `rowSpan`) 및 셀 너비/높이를 재귀적으로 파싱하고 결측 셀(`covered`)을 자동 유도하여 깨짐 없는 표 렌더링을 보장합니다.

### 2. Pure Rust 네이티브 레거시 HWP 5.x 파서 (`wordcraft-formats::hwp5`)
- **OLE2 Compound File 디코딩:** Rust의 `cfb` 크레이트를 통해 OLE 복합 바이너리 컨테이너의 `FileHeader`, `DocInfo`, `BodyText/Section*` 스트림을 안전하게 탐색합니다.
- **Deflate/zlib 고속 압축 해제:** `miniz_oxide`를 이용해 압축된 본문 스트림을 인메모리에서 무손실 해제합니다.
- **계층적 레코드 태그 파서:** `HWPTAG_PARA_HEADER`, `HWPTAG_PARA_TEXT`(UTF-16LE 텍스트), `HWPTAG_PARA_CHAR_SHAPE`(글자 모양 런 분할), `HWPTAG_PAGE_DEF`(편집 용지 및 여백)를 완벽하게 파싱하여 WordCraft 문서 모델로 변환합니다.

### 3. "HWP vs Word" 듀얼 인터페이스 및 키맵 전환 시스템
사용자의 습관과 선호도에 따라 단 한 번의 클릭 또는 환경 설정에서 즉시 **MS Word 모드**와 **아래아한글 모드**를 실시간으로 전환할 수 있습니다.

#### A. 단축키 매핑 비교
| 주요 기능 | Word 스타일 (`KeymapPreset::Word`) | HWP 스타일 (`KeymapPreset::Hwp`) |
| :--- | :--- | :--- |
| **편집 용지 설정 (Page Setup)** | 리본 메뉴 레이아웃 | **`F7`** |
| **글자 모양 (Font Dialog)** | `Ctrl + D` | **`Alt + L`** |
| **문단 모양 (Paragraph Dialog)** | `Alt + H, P, G` | **`Alt + T`** |
| **저장 (Save)** | `Ctrl + S` | **`Alt + S`** (Ctrl+S 병행) |
| **불러오기 (Open)** | `Ctrl + O` | **`Alt + O`** |
| **새 문서 (New)** | `Ctrl + N` | **`Alt + N`** |
| **인쇄 (Print)** | `Ctrl + P` | **`Alt + P`** |
| **문자표 / 기호 (Symbol)** | 기호 대화상자 | **`Ctrl + F10`** |
| **모양 복사 (Format Painter)** | `Ctrl + Shift + C` | **`Alt + C`** |

#### B. 2단계 연쇄 단축키 (Chord State Machine) 지원
HWP 특유의 `Ctrl + N` 계열 조합 키를 완벽 지원합니다:
- **`Ctrl + N` 입력 후 `T`:** 표 만들기 대화상자 실행
- **`Ctrl + N` 입력 후 `I`:** 그림 삽입 대화상자 실행
- **`Ctrl + N` 입력 후 `F`:** 각주 삽입
- **`Ctrl + N` 입력 후 `H`:** 머리글 삽입

#### C. 표(Table) 내부 핫키 지원
커서가 표 안에 위치하고 셀 블록이 선택되었을 때, 타이핑과 충돌 없이 전용 핫키가 동작합니다:
- **`F5`:** 1회 누름 시 현재 셀 선택, 2회 누름 시 다중 셀 확장, 3회 누름 시 표 전체 선택
- **`M`:** 선택된 셀 합치기 (Merge Cells)
- **`S`:** 셀 나누기 (Split Cells) 대화상자 호출
- **`C`:** 셀 배경색 및 테두리 설정 (Color / Shading)
- **`L`:** 셀 선 모양 설정 (Line / Borders)

#### D. HWP 스타일 8대 메뉴 탭 구성
상단 리본 인터페이스가 한글 사용자에게 익숙한 구조로 재배치됩니다:
```
[파일]  [편집]  [보기]  [입력]  [서식]  [쪽]  [검토]  [도구]  |  [표 디자인/레이아웃]
```
- **파일:** 새로 만들기, 불러오기, 저장, 다른 이름으로 저장, 인쇄, 문서 정보
- **편집:** 실행 취소, 다시 실행, 오려두기, 복사, 붙여넣기, 모양 복사, 찾기/바꾸기
- **보기:** 쪽 윤곽(인쇄 레이아웃), 읽기 모드, 눈금자, 다크 페이지, 확대/축소
- **입력:** 표(Ctrl+N, T), 그림(Ctrl+N, I), 수식, 문자표(Ctrl+F10), 각주
- **서식:** 글자 모양(Alt+L), 문단 모양(Alt+T), 스타일 갤러리
- **쪽:** 편집 용지(F7), 여백, 다단, 구역 나누기, 머리글/꼬리말, 쪽 번호
- **검토:** 맞춤법 검사, 변경 내용 추적, 메모
- **도구:** 단축키 설정, 환경 설정

---

## 🏛️ 시스템 아키텍처 (System Architecture)

```
KimchiCraft Workspace
├── crates/
│   ├── formats/                     # 문서 입출력 크레이트
│   │   ├── src/hwpx/                # [신규] HWPX 네이티브 파서 모듈 (XML/ZIP)
│   │   │   ├── styles.rs            # HWPU -> pt 단위 변환, 색상/정렬 파싱
│   │   │   ├── header.rs            # Contents/header.xml (폰트, 스타일, charPr, paraPr)
│   │   │   ├── package.rs           # ZIP 컨테이너, manifest, BinData 관리
│   │   │   ├── section.rs           # Contents/section*.xml (단락, 텍스트, 표, 셀 병합)
│   │   │   ├── mod.rs               # import, import_on 진입점
│   │   │   └── tests.rs             # HWPX 단위 테스트
│   │   ├── src/hwp5/                # [신규] 레거시 HWP 5.x 네이티브 파서 모듈 (OLE2)
│   │   │   ├── record.rs            # HWP 계층형 레코드 헤더 및 스트림 리더
│   │   │   ├── header.rs            # FileHeader 검증 및 DocInfo 파싱
│   │   │   ├── section.rs           # BodyText 스트림 파서 (UTF-16LE 텍스트 런 및 여백)
│   │   │   ├── mod.rs               # cfb OLE2 복합 파일 읽기 및 압축 해제
│   │   │   └── tests.rs             # HWP 5.x 단위 테스트
│   │   └── src/lib.rs               # "hwpx" 및 "hwp" 포맷 등록 및 파이프라인 연결
│   │
│   └── ui-egui/                     # egui 기반 UI 및 이벤트 처리 크레이트
│       ├── src/preset.rs            # [신규] AppPreset, KeymapPreset, UiPreset 정의
│       ├── src/keys/
│       │   ├── chord.rs             # [신규] Ctrl+N, T 등 2단계 연쇄 키 상태 머신
│       │   ├── shortcuts.rs         # HWP 전용 단축키(F7, Alt+L, Alt+T 등) 디스패처
│       │   └── canvas.rs            # Chord 처리 및 표 내부 핫키(F5, S, M, C, L) 처리
│       ├── src/ribbon/
│       │   ├── hwp.rs               # [신규] HWP 스타일 8대 탭 렌더러
│       │   └── tabs.rs              # 탭 스트립 레이아웃 및 HWP_TABS 지원
│       └── src/options.rs           # General 환경 설정에 UI & 단축키 모드 선택기 추가
└── apps/
    └── wordcraft/                   # 데스크톱 메인 애플리케이션 진입점
```

---

## 🚀 단계별 개발 로드맵 (Roadmap)

- [x] **Phase 1: HWPX 네이티브 파서 및 공통 문서 AST 매핑**
  - ZIP 아카이브 구조 안전 로딩 및 무결성 검증
  - `header.xml` 기반 글꼴, 글자/문단 스타일 해석
  - `section*.xml` 기반 단락 및 셀 병합 표 구조 복원
  - 단위 테스트 및 파싱 검증 완료
- [x] **Phase 2: HWP vs Word 듀얼 모드 UI 및 키맵 시스템**
  - `KeymapPreset` 및 `UiPreset` 데이터 모델 구축
  - HWP 핵심 단축키(F7, Alt+L, Alt+T, Alt+S, Alt+O, Alt+N, Ctrl+F10) 바인딩
  - 2단계 연쇄 단축키(`Ctrl+N`, `T`) Chord 상태 머신 구현
  - 표 셀 선택 상태(`F5`) 및 표 편집 핫키(`M`, `S`, `C`, `L`) 가로채기
  - HWP 스타일 8대 리본 탭 레이아웃 구현 및 실시간 전환 스위치 연결
- [x] **Phase 3: 레거시 HWP 5.x 바이너리 포맷 읽기 지원**
  - OLE2 Compound File 컨테이너(`cfb`) 기반 스트림 파서 연결
  - `BodyText/Section*` Deflate 압축 해제 및 레코드 태그 디코딩
  - UTF-16LE 텍스트 런 및 문단 모양 복원
- [ ] **Phase 4: HWPX 내보내기 (Export / Round-trip)**
  - 편집된 문서 AST를 KS X 6101 XML 규격으로 직렬화하여 `.hwpx`로 저장

---

## 🛠️ 빌드 및 실행 방법 (Build & Run)

### 요구 사양
- **Rust:** 최신 stable 버전 (1.95+ 권장)
- **Git**

### 1. 저장소 클론
```bash
git clone https://github.com/RR-korea/kimchicraft.git
cd kimchicraft
```

### 2. 데스크톱 앱 실행
```bash
cargo run --bin wordcraft
```

### 3. 테스트 실행
```bash
# HWPX 파서 테스트
cargo test -p wordcraft-formats --lib hwpx

# HWP 5.x 파서 테스트
cargo test -p wordcraft-formats --lib hwp5
```

---

## 📜 라이선스 (License)

KimchiCraft는 원본 WordCraft 프로젝트의 라이선스를 계승하여 **MIT 라이선스** 또는 **Apache-2.0 라이선스**의 듀얼 라이선스로 배포됩니다.
자세한 내용은 `LICENSE-MIT` 및 `LICENSE-APACHE` 파일을 참고하세요.

---

<p align="center">
  <b>KimchiCraft</b> - Made with ❤️ for the Korean open-source community by RR-korea.
</p>

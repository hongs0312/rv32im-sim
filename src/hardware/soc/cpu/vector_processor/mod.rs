pub mod components;
pub mod pipeline_stage;

use components::lane::{Sew, VectorOp, VectorUnit};
use components::viq::VectorInstructionQueue;
use components::vrf::{VLEN_BYTES, Vrf};
use pipeline_stage::{VExMemRegister, VIdExRegister, VMemWbRegister};

pub struct VectorProcessor {
    // 하드웨어 부품들
    pub vrf: Vrf,
    pub v_unit: VectorUnit,

    // 💡 스코어보드: 각 레지스터(v0~v31)가 현재 파이프라인에서 사용 중인지 추적
    pub scoreboard: [bool; 32],

    // 파이프라인 래치 (레지스터)
    pub id_ex: VIdExRegister,
    pub ex_mem: VExMemRegister,
    pub mem_wb: VMemWbRegister,
}

impl VectorProcessor {
    pub fn new() -> Self {
        Self {
            vrf: Vrf::new(),
            v_unit: VectorUnit::new(),
            scoreboard: [false; 32],

            id_ex: Default::default(),
            ex_mem: Default::default(),
            mem_wb: Default::default(),
        }
    }

    /// 벡터 코어의 메인 클럭. CPU(스칼라)의 tick() 안에서 매 사이클 호출됩니다.
    pub fn tick(&mut self, viq: &mut VectorInstructionQueue) {
        // -----------------------------------------------------
        // [Phase 1: 조합 논리 평가] 파이프라인을 역순으로 실행!
        // -----------------------------------------------------

        // 1. V-WB (Writeback): 상태를 가장 먼저 비워줘야 이전 단계가 스톨되지 않음
        self.run_wb();

        // 2. V-MEM (Memory): EX 단계의 결과를 받아옴 (현재는 산술 연산이라 단순 패스스루)
        let next_mem_wb = self.run_mem();

        // 3. V-EX (Execute): 벡터 연산기 가동
        let next_ex_mem = self.run_ex();

        // 4. V-ID (Decode/Issue): VIQ에서 명령어를 뽑고 VRF 읽기
        let next_id_ex = self.run_id(viq);

        // -----------------------------------------------------
        // [Phase 2: 순차 논리 업데이트] 클럭 엣지에 레지스터 업데이트
        // -----------------------------------------------------
        self.mem_wb = next_mem_wb;
        self.ex_mem = next_ex_mem;
        self.id_ex = next_id_ex;

        // VRF에 걸린 쓰기 포트(V-WB에서 걸어둔 값)를 실제 메모리에 커밋
        self.vrf.tick();
    }

    // -----------------------------------------------------
    // 파이프라인 각 단계별 서브루틴
    // -----------------------------------------------------

    fn run_wb(&mut self) {
        if self.mem_wb.valid {
            // VRF 쓰기 포트에 기록 요청
            self.vrf.write_port(self.mem_wb.vd_idx, self.mem_wb.wb_data);
            // 💡 스코어보드 해제: 이제 이 레지스터를 다른 명령어가 써도 됨!
            self.scoreboard[self.mem_wb.vd_idx] = false;
        }
    }

    fn run_mem(&self) -> VMemWbRegister {
        if self.ex_mem.valid {
            // TODO: 나중에 메모리 로드/스토어 명령어일 경우 SystemBus를 찌르는 로직 추가
            // 지금은 산술 연산이므로 EX 결과를 그대로 WB로 토스
            VMemWbRegister {
                valid: true,
                wb_data: self.ex_mem.alu_result,
                vd_idx: self.ex_mem.vd_idx,
            }
        } else {
            VMemWbRegister::default()
        }
    }

    fn run_ex(&self) -> VExMemRegister {
        if self.id_ex.valid {
            // 💡 Lane.rs에 만들어둔 4차선 병렬 벡터 유닛 통과!
            let result = self.v_unit.execute(
                self.id_ex.op,
                self.id_ex.vs1_data,
                self.id_ex.vs2_data,
                self.id_ex.sew,
            );
            VExMemRegister {
                valid: true,
                alu_result: result,
                vd_idx: self.id_ex.vd_idx,
            }
        } else {
            VExMemRegister::default()
        }
    }

    fn run_id(&mut self, viq: &mut VectorInstructionQueue) -> VIdExRegister {
        // VIQ에 대기 중인 명령어가 있는지 엿보기(peek)
        if let Some(packet) = viq.peek() {
            // 1. 임시 디코딩 (Opcode 파싱 - 여기서는 가상의 디코딩 함수라 가정)
            let vs1_idx = 1; // 파싱된 src1 레지스터 번호
            let vs2_idx = 2; // 파싱된 src2 레지스터 번호
            let vd_idx = 3; // 파싱된 dest 레지스터 번호

            // 2. 💡 스코어보드 해저드 검사 (RAW, WAW 방지)
            if self.scoreboard[vs1_idx] || self.scoreboard[vs2_idx] || self.scoreboard[vd_idx] {
                // 누군가 레지스터를 쓰고 있다면? 큐에서 뽑지 않고 여기서 Stall!
                return VIdExRegister::default();
            }

            // 3. 해저드가 없으면 VIQ에서 완전히 뽑아냄(pop)
            viq.pop();

            // 4. 목적지 레지스터를 스코어보드에 '사용 중(Busy)'으로 잠금
            self.scoreboard[vd_idx] = true;

            // 5. VRF에서 데이터 슥싹 읽어오기
            return VIdExRegister {
                valid: true,
                op: VectorOp::Add, // 임시 하드코딩
                sew: Sew::E32,     // 임시 하드코딩
                vs1_data: self.vrf.read(vs1_idx),
                vs2_data: self.vrf.read(vs2_idx),
                vd_idx,
            };
        }
        VIdExRegister::default()
    }
}

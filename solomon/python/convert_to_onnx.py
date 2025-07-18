# -*- coding: utf-8 -*-
"""
학습된 모델을 ONNX 형식으로 변환하여 Rust에서 직접 사용할 수 있도록 합니다.
"""
import json
import logging
import sqlite3
from datetime import datetime
from pathlib import Path
from typing import List

import numpy as np
import onnx
import onnxruntime as ort
import pandas as pd
from joblib import load
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType

# 로깅 설정
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)


class ONNXConverter:
    """학습된 모델을 ONNX 형식으로 변환하는 클래스"""

    def __init__(self, db_path: str = "D:/db/solomon.db"):
        self.db_path = db_path
        self.model = None
        self.features: List[str] = []
        self.metadata = None

    def load_model_and_metadata(self, model_path: str, metadata_path: str = None):
        # 모델 로드
        self.model = load(model_path)
        logger.info(f"모델 로드 완료: {model_path}")

        # 메타데이터 로드
        metadata_path = metadata_path or model_path.replace('.joblib', '_metadata.json')
        if Path(metadata_path).exists():
            with open(metadata_path, 'r', encoding='utf-8') as f:
                self.metadata = json.load(f)
            self.features = self.metadata.get('features', [])
            logger.info(f"메타데이터 로드 완료: {metadata_path} (특성 수: {len(self.features)})")
        else:
            logger.warning(f"메타데이터 파일을 찾을 수 없습니다: {metadata_path}")
            self.features = self._load_features_from_file()

    @staticmethod
    def _load_features_from_file(features_file: str = "features.txt") -> List[str]:
        if not Path(features_file).exists():
            raise FileNotFoundError(f"특성 파일을 찾을 수 없습니다: {features_file}")
        with open(features_file, 'r', encoding='utf-8') as f:
            features = [line.strip() for line in f if line.strip()]
        logger.info(f"features.txt에서 로드된 특성 수: {len(features)}")
        return features

    def load_sample_data_for_conversion(self, limit: int = 1000) -> pd.DataFrame:
        conn = sqlite3.connect(self.db_path)
        if not self.features:
            raise ValueError("변환할 특성이 설정되지 않았습니다.")
        # 테이블 조인 쿼리 생성
        mapping = {}
        for feat in self.features:
            table = feat.split('_')[0]
            col = feat.replace(f"{table}_", '')
            alias = 'a' if table == 'day1' else 'b' if table == 'day2' else 'c' if table == 'day3' else 'd'
            mapping[feat] = f"{alias}.{col} AS {feat}"
        cols = ', '.join(mapping[f] for f in self.features)
        sql = f"""
SELECT {cols}
FROM day1 a
JOIN day2 b ON a.date=b.date AND a.stock_code=b.stock_code
JOIN day3 c ON a.date=c.date AND a.stock_code=c.stock_code
JOIN day4 d ON a.date=d.date AND a.stock_code=d.stock_code
JOIN answer_v3 e ON CAST(REPLACE(a.date,'-','') AS INTEGER)=e.date AND a.stock_code=e.stock_code
WHERE e.date<20230601
ORDER BY a.date, a.stock_code
LIMIT {limit}
"""
        df = pd.read_sql_query(sql, conn)
        conn.close()
        df = df[self.features].apply(pd.to_numeric, errors='coerce')
        df = df.fillna(df.mean()).astype('float32')
        logger.info(f"샘플 데이터 로드 완료: {len(df)} 행")
        return df

    def convert_to_onnx(self, output_path: str = "models/best_model.onnx") -> str:
        if self.model is None:
            raise ValueError("먼저 모델을 로드하세요.")

        sample_df = self.load_sample_data_for_conversion(limit=100)
        initial_type = [('input', FloatTensorType([None, len(self.features)]))]

        logger.info("ONNX 변환 시작...")
        onx = convert_sklearn(
            self.model,
            initial_types=initial_type,
            target_opset=12,
            options={type(self.model): {'zipmap': False}}
        )
        Path(output_path).parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, 'wb') as f:
            f.write(onx.SerializeToString())
        logger.info(f"ONNX 모델 저장됨: {output_path}")

        self._validate_onnx(output_path, sample_df)
        return output_path

    def _validate_onnx(self, onnx_path: str, sample_df: pd.DataFrame) -> bool:
        sess = ort.InferenceSession(onnx_path)
        inp = sess.get_inputs()[0]
        outs = sess.get_outputs()
        # 확률 출력으로 추정되는 tensor(float)[None, n]
        prob_out = next((o for o in outs if 'float' in o.type and len(o.shape) == 2), None)
        if not prob_out:
            logger.error("확률 출력 텐서를 찾을 수 없습니다.")
            return False
        logger.info(f"출력 선택: {prob_out.name} ({prob_out.type} {prob_out.shape})")

        arr = sample_df.values.astype(np.float32)
        orig = self.model.predict_proba(arr)
        onnx_pred = sess.run([prob_out.name], {inp.name: arr})[0]

        if onnx_pred.dtype != np.float32:
            logger.warning(f"출력 dtype 불일치: {onnx_pred.dtype}")
        if onnx_pred.shape != orig.shape:
            logger.warning(f"출력 shape 불일치: {onnx_pred.shape} vs {orig.shape}")

        diff = np.abs(orig - onnx_pred).max()
        if diff < 1e-5:
            logger.info(f"검증 성공! 최대 차이: {diff:.2e}")
            return True
        logger.warning(f"검증 경고! 최대 차이: {diff:.2e}")
        return False

    def save_metadata(self, onnx_path: str, output_dir: str = "models") -> str:
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        meta = {
            'model_type': 'onnx',
            'model_path': onnx_path,
            'features': self.features,
            'feature_count': len(self.features),
            'converted_at': datetime.now().isoformat(),
            'original': self.metadata
        }
        out = f"{output_dir}/onnx_model_metadata.json"
        with open(out, 'w', encoding='utf-8') as f:
            json.dump(meta, f, ensure_ascii=False, indent=2)
        logger.info(f"메타데이터 저장됨: {out}")
        return out

    def create_rust_files(self, onnx_path: str, output_dir: str = "models") -> str:
        sess = ort.InferenceSession(onnx_path)
        inp = sess.get_inputs()[0]
        outs = sess.get_outputs()
        prob_out = next((o for o in outs if 'float' in o.type and len(o.shape) == 2), outs[0])

        # null 값을 실제 숫자로 변환
        input_shape = [1 if x is None else x for x in inp.shape]
        output_shape = [1 if x is None else x for x in prob_out.shape]

        info = {
            'onnx_model_path': onnx_path,
            'features': self.features,
            'feature_count': len(self.features),
            'input_name': inp.name,
            'input_shape': input_shape,
            'output_name': prob_out.name,
            'output_shape': output_shape
        }
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        out = f"{output_dir}/rust_model_info.json"
        with open(out, 'w', encoding='utf-8') as f:
            json.dump(info, f, ensure_ascii=False, indent=2)
        logger.info(f"Rust 통합 파일 생성됨: {out}")
        return out


def main():
    # 설정 파일
    cfg = Path("config_onnx_conversion.json")
    if not cfg.exists():
        default = {
            'model_path': 'best_model.joblib',
            'metadata_path': None,
            'db_path': 'D:/db/solomon.db',
            'onnx_output_path': 'models/best_model.onnx',
            'output_dir': 'models'
        }
        cfg.write_text(json.dumps(default, ensure_ascii=False, indent=2), encoding='utf-8')
        logger.info(f"기본 config 파일 생성됨: {cfg}")
        return

    config = json.loads(cfg.read_text(encoding='utf-8'))
    conv = ONNXConverter(config['db_path'])
    conv.load_model_and_metadata(config['model_path'], config.get('metadata_path'))
    onnx_path = conv.convert_to_onnx(config['onnx_output_path'])
    conv.save_metadata(onnx_path, config['output_dir'])
    conv.create_rust_files(onnx_path, config['output_dir'])


if __name__ == '__main__':
    main()

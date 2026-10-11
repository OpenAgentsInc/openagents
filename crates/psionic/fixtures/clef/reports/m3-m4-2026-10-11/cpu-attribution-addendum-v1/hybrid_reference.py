#!/usr/bin/env python3
"""One-case same-GGUF CPU reference with only FFN matrices replaced by original weights."""
import argparse
import gc
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import sys
import textwrap
import time
import types

for name in ("OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS","VECLIB_MAXIMUM_THREADS","RAYON_NUM_THREADS"):
    os.environ[name]="2"
from gguf_import import BASE, GGUF, fixture_checks, layer_arrays, ordinary_norm_to_hf, require, sha_bytes
from hybrid_import import HybridGguf

HERE=Path(__file__).resolve().parent
ROOT=Path("<OPENAGENTS_WORKTREE>")
HF=BASE / "artifacts/clef-bf16"
CLEAN=BASE / "reference27b/cpu-head-stage1/results"
MODELING_SHA="fd9963c32aa3d50c753a990a2670cfdbea055b6316b40027e0de976019e42551"
SOURCE_SHA="0e304cf7c6500e8bb59bef7e2afd2c6373f82596dfb3b57d1aa93c175e2dc3a3"
HEAD_SHA="a010ac04f078e699988e4049cbea5e62c962393f59fec366640b64e8d69a4953"


def file_sha(path):
    digest=hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda:f.read(1024**2),b""):digest.update(block)
    return digest.hexdigest()


def write(path,value):
    temporary=path.with_suffix(path.suffix+".partial")
    temporary.write_text(json.dumps(value,indent=2,allow_nan=False)+"\n")
    temporary.replace(path)


def tensor_sha(torch,tensor):
    require(tensor.device.type=="cpu","CPU-only tensor required")
    return sha_bytes(tensor.detach().contiguous().view(torch.uint8).numpy().tobytes())


def make_negative_a_adapter(modeling):
    source=textwrap.dedent(inspect.getsource(modeling.Qwen3_5GatedDeltaNet.forward))
    original="-self.A_log.float().exp()"
    require(source.count(original)==1,"official negative-A expression changed")
    adapted=source.replace(original,"self.gguf_negative_a.float()")
    scope=dict(vars(modeling))
    exec(compile(adapted,"<stored-negative-A adapter>","exec"),scope)
    return scope["forward"],{"original_forward_sha256":sha_bytes(source.encode()),
            "adapted_forward_sha256":sha_bytes(adapted.encode()),"replaced_expression":original,
            "replacement":"self.gguf_negative_a.float()",
            "reason":"GGUF stores negative exp(A_log). Use those exact decoded values without a lossy log/exp round trip.",
            "other_operator_changes":False},adapted


def plan(reader,tokens):
    layers=[]
    for index in range(64):
        tensors={name:item for name,item in reader.tensors.items() if name.startswith(f"blk.{index}.")}
        size=sum(__import__("math").prod(item["shape"])*4 for item in tensors.values())
        matrix_elements=sum(__import__("math").prod(item["shape"]) for name,item in tensors.items()
                            if len(item["shape"])==2 and name!="blk.%d.ssm_conv1d.weight"%index)
        layers.append({"layer":index,"weight_f32_bytes":size,"dense_gemm_flops":2*tokens*matrix_elements})
    return {"layers":layers,"maximum_layer_f32_bytes":max(row["weight_f32_bytes"] for row in layers),
            "total_dense_gemm_flops":sum(row["dense_gemm_flops"] for row in layers),
            "input_tokens":tokens,"hidden_rows_bytes":tokens*5120*4,
            "temporary_decode_blocks":2048,"largest_quantized_read_bytes":2048*210,
            "maximum_single_tensor_f32_bytes":512*1024**2,"owned_group_cap_gib":12,
            "start_available_gib":24,"available_floor_gib":12,"threads":2,
            "runtime_estimate":"Unmeasured: several minutes to roughly30minutes including CPU GEMMs, DeltaNet and19GB sequential tensor reads; guard deadline1800s. First-layer timing must precede approval of the full pass.",
            "numerical_scope":"Exact decoded GGUF values in CPU f32; excludes native Metal fp16 activation/weight rounding."}


def run(args):
    resources=set(os.environ.get("OPENAGENTS_LEASES","").split(","))
    require("memory" in resources and not resources.intersection({"gpu","quiet"}),"requires memory lease, no GPU or quiet")
    require(os.environ.get("CLEF_STREAMED_HYBRID_GUARDED")=="1","run through the owned CPU guard")
    require(not args.output_dir.exists(),"preserve previous output")
    args.output_dir.mkdir(parents=True)
    receipt={"kind":"streamed_same_gguf_original_ffn_cpu_f32_text014","status":"validating","completed":False,
             "device":"cpu","dtype":"float32","threads":2,"layers":[],"diagnostic_only":True,
             "model_allocation":"One decoder layer at a time; selected input/output embedding rows only.",
             "gguf_sha256_from_prior_verification":"9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822",
             "script_sha256":file_sha(Path(__file__)),"importer_sha256":file_sha(HERE / "hybrid_import.py")}
    report=args.output_dir / "receipt.json"
    write(report,receipt)
    reader=None
    try:
        import torch
        import transformers
        from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling
        torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device("cpu")
        require(str(torch.__version__)=="2.9.1" and transformers.__version__=="5.10.2","runtime version differs")
        require(file_sha(Path(modeling.__file__))==MODELING_SHA,"official model operators changed")
        require(modeling.FusedRMSNormGated is None and modeling.causal_conv1d_fn is None
                and modeling.chunk_gated_delta_rule is None and modeling.fused_recurrent_gated_delta_rule is None,
                "unexpected optional GPU kernels; use the verified pure PyTorch environment")
        require(file_sha(HF / "joint_schema_model.py")==SOURCE_SHA and file_sha(HF / "joint_head.safetensors")==HEAD_SHA,
                "official head artifacts differ")
        config_raw=json.loads((HF / "config.json").read_text())["text_config"]
        require(config_raw["num_hidden_layers"]==64 and config_raw["hidden_size"]==5120 and config_raw["intermediate_size"]==17408,
                "27B architecture differs")
        config=modeling.Qwen3_5TextConfig.from_dict(config_raw)
        config._attn_implementation="eager";config.use_cache=False;config.dtype=torch.float32
        clean=json.loads((CLEAN / "reference.json").read_text())
        case=json.loads((CLEAN / "text-014.json").read_text())
        require(clean["completed"] and clean["all_immutable_checks_passed"] and case in clean["cases"],"clean case differs")
        require(case["immutable_checks"]["all_exact"] and len(case["input_ids"])==321,"text014 token identity differs")
        ids=case["input_ids"]
        spec=importlib.util.spec_from_file_location("streamed_official_head",HF / "joint_schema_model.py")
        official=importlib.util.module_from_spec(spec);sys.modules[spec.name]=official;spec.loader.exec_module(official)
        processor=transformers.AutoProcessor.from_pretrained(HF,local_files_only=True)
        encoded=official.encode_record(processor.tokenizer,json.loads(case["request_body"]),processor=processor)
        from dataclasses import asdict
        require(json.loads(json.dumps(asdict(encoded)))==case["records"][0],"fresh exact token IDs/spans/options differ")
        reader=HybridGguf()
        receipt.update(plan=plan(reader,len(ids)),fixtures=fixture_checks(),torch=str(torch.__version__),
                       transformers=transformers.__version__,gguf_file_state=reader.state,
                       independent_decoder=reader.decoder_identity,clean_reference_sha256=file_sha(CLEAN / "reference.json"),
                       clean_case_sha256=file_sha(CLEAN / "text-014.json"),input_ids=ids,records=case["records"])
        adapter,adapter_identity,adapter_source=make_negative_a_adapter(modeling)
        (args.output_dir / "negative_a_forward.py").write_text(adapter_source)
        receipt["negative_a_adapter"]=adapter_identity
        receipt["matrix_sources"]=reader.matrix_sources
        receipt["other_weights"]="Unchanged GGUF input/output embeddings, attention/DeltaNet projections, norms, convolution, and stored negative-A."
        receipt["original_lexical_cross_swap"]="Diagnostic only; backbone run changes only the192 FFN matrices."
        hidden=torch.from_numpy(reader.rows("token_embd.weight",ids)).unsqueeze(0)
        require(hidden.shape==(1,321,5120),"embedding shape differs")
        position_ids=torch.arange(len(ids),device="cpu").view(1,-1)
        positions=position_ids[None,...].expand(3,1,-1)
        rotary=modeling.Qwen3_5TextRotaryEmbedding(config,device=torch.device("cpu"))
        with torch.inference_mode():
            position_embeddings=rotary(hidden,positions)
            causal_mask=modeling.create_causal_mask(config=config,inputs_embeds=hidden,
                attention_mask=torch.ones((1,len(ids)),dtype=torch.long),past_key_values=None,position_ids=position_ids)
        limit=1 if args.first_layer_only else 64
        receipt.update(status="running",requested_layers=limit)
        write(report,receipt)
        from safetensors.torch import save_file,load_file
        for index in range(limit):
            reader.check_state()
            began=time.monotonic()
            with torch.device("meta"):
                layer=modeling.Qwen3_5DecoderLayer(config,index)
            arrays,negative_a=layer_arrays(reader,index)
            require(set(arrays)==set(layer.state_dict()),"layer parameter names differ")
            state={name:torch.from_numpy(value) for name,value in arrays.items()}
            layer.load_state_dict(state,strict=True,assign=True)
            if negative_a is not None:
                layer.linear_attn.register_buffer("gguf_negative_a",torch.from_numpy(negative_a))
                layer.linear_attn.forward=types.MethodType(adapter,layer.linear_attn)
            del arrays,state,negative_a
            layer.eval()
            require(all(p.device.type=="cpu" and p.dtype==torch.float32 for p in layer.parameters()),"layer left meta/CPU-f32 bounds")
            load_seconds=time.monotonic()-began
            before=tensor_sha(torch,hidden)
            began=time.monotonic()
            with torch.inference_mode():
                output=layer(hidden,position_embeddings=position_embeddings,
                    attention_mask=causal_mask if index%4==3 else None,position_ids=position_ids,
                    past_key_values=None,use_cache=False)
            require(tensor_sha(torch,hidden)==before,"decoder layer mutated its input")
            require(output.shape==hidden.shape and bool(torch.isfinite(output).all()),"invalid layer output")
            elapsed=time.monotonic()-began
            hidden=output;del output,layer;gc.collect()
            reader.check_state()
            evidence=args.output_dir / f"layer-{index:02d}.safetensors"
            save_file({"hidden":hidden.contiguous()},str(evidence))
            row={"layer":index,"load_decode_seconds":load_seconds,"forward_seconds":elapsed,
                 "hidden_sha256":tensor_sha(torch,hidden),"file":evidence.name,"file_sha256":file_sha(evidence),
                 "finite":True,"input_unchanged":True,"payload_read_bytes":reader.payload_read_bytes}
            receipt["layers"].append(row);write(report,receipt)
            print(json.dumps(row),flush=True)
        if args.first_layer_only:
            receipt.update(status="first_layer_complete",completed=False,full_pass_executed=False)
            return
        norm=modeling.Qwen3_5RMSNorm(5120,eps=config.rms_norm_eps)
        norm.weight=torch.nn.Parameter(torch.from_numpy(ordinary_norm_to_hf(reader.full("output_norm.weight"))),requires_grad=False)
        with torch.inference_mode():hidden=norm(hidden)
        require(bool(torch.isfinite(hidden).all()),"nonfinite final rows")
        save_file({"hidden":hidden.contiguous()},str(args.output_dir / "final-hidden.safetensors"))
        (args.output_dir / "hidden.f32").write_bytes(hidden.numpy().astype("<f4").tobytes())
        tokens=sorted({token for q in encoded.questions for first,end in q.option_spans for token in ids[first:end]})
        lexical=torch.from_numpy(reader.rows("output.weight",tokens))
        remap={token:index for index,token in enumerate(tokens)}
        head_ids=[remap.get(token,0) for token in ids]
        record=official.EncodedRecord(input_ids=tuple(head_ids),questions=encoded.questions,record_id=encoded.record_id)
        head=official.JointSchemaHead(**json.loads((HF / "joint_head_config.json").read_text()))
        head.load_state_dict(load_file(str(HF / "joint_head.safetensors"),device="cpu"),strict=True)
        head=head.to(device="cpu",dtype=torch.float32).eval()
        require(len(reader.replaced)==192 and len(set(reader.replaced))==192,"FFN replacement coverage differs")
        original_lexical=torch.from_numpy(reader.original.rows("lm_head.weight",tokens))
        quantized=load_file(str(HERE / "full-1/results/head-inputs.safetensors"),device="cpu")
        require(quantized["lexical_token_ids"].tolist()==tokens and quantized["remapped_input_ids"].tolist()==[head_ids],"quantized token mapping differs")
        before_hidden,before_lexical=tensor_sha(torch,hidden),tensor_sha(torch,lexical)
        comparisons={}
        with torch.inference_mode():
            for label,rows,words in (("original_ffn_q6_lexical",hidden,lexical),("original_ffn_original_lexical",hidden,original_lexical),
                                     ("q4_hidden_q6_lexical",quantized["hidden"],lexical),("q4_hidden_original_lexical",quantized["hidden"],original_lexical)):
                prior_rows,prior_words=tensor_sha(torch,rows),tensor_sha(torch,words)
                values=head(rows,torch.tensor([head_ids],dtype=torch.long),torch.ones((1,len(ids)),dtype=torch.long),[record],words)[0]
                require(tensor_sha(torch,rows)==prior_rows and tensor_sha(torch,words)==prior_words,"cross-swap head inputs changed")
                comparisons[label]=[value.float().tolist() for value in values]
        require(tensor_sha(torch,hidden)==before_hidden and tensor_sha(torch,lexical)==before_lexical,"head inputs changed")
        receipt.update(status="complete",completed=True,full_pass_executed=True,hidden_sha256=before_hidden,lexical_sha256=before_lexical,
                       logits=comparisons["original_ffn_q6_lexical"],head_cross_swaps=comparisons,payload_read_bytes=reader.payload_read_bytes,
                       quantized_payload_bytes=reader.quantized.payload_read_bytes,original_payload_bytes=reader.original.payload_read_bytes,
                       original_ffn_matrices_loaded=len(reader.replaced))
        save_file({"hidden":hidden.contiguous(),"lexical":lexical,"input_ids":torch.tensor([ids]),
                   "lexical_token_ids":torch.tensor(tokens),"remapped_input_ids":torch.tensor([head_ids])},
                  str(args.output_dir / "head-inputs.safetensors"))
    except BaseException as error:
        receipt.update(status="failed",error_type=type(error).__name__,error=str(error))
        raise
    finally:
        if reader is not None:reader.close()
        write(report,receipt)


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir",type=Path,required=True)
    parser.add_argument("--first-layer-only",action="store_true")
    run(parser.parse_args())

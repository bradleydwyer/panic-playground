#!/usr/bin/env python3
"""Recover Panic's FCP/IV startup by isolated 16-bit Unicorn emulation.

This models only the protection loader, never executes the DOS target on the
host, and records assumptions. It is not a PC hardware or demo emulator.
"""
from pathlib import Path
import argparse, collections, hashlib, json, struct
from unicorn import *
from unicorn.x86_const import *
from capstone import Cs, CS_ARCH_X86, CS_MODE_16

BASE = 0x1000
REGS = {n: globals()['UC_X86_REG_' + n.upper()] for n in
        ['ax','bx','cx','dx','si','di','bp','sp','cs','ds','es','ss','ip','eflags']}

def parse_mz(data):
    """Validate the initialized DOS module and relocations before emulation."""
    if len(data)<28 or data[:2]!=b'MZ':raise ValueError('Missing or truncated DOS MZ header')
    h=struct.unpack_from('<14H',data)
    if h[2]==0 or h[1]>511:raise ValueError('Invalid MZ page count or final-page length')
    size=(h[2]-1)*512+(h[1] or 512); header=h[4]*16
    if not 28<=header<=size<=len(data):raise ValueError('MZ initialized module outside input')
    if h[3] and not 28<=h[12]<=h[12]+h[3]*4<=header:raise ValueError('MZ relocation table outside header')
    for i in range(h[3]):
        off,seg=struct.unpack_from('<HH',data,h[12]+i*4)
        if seg*16+off+2>size-header:raise ValueError('MZ relocation destination outside module')
    return h,size,header

def main():
    global BASE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=Path('artifacts/original/release/PANIC.EXE'))
    parser.add_argument('--output', type=Path, default=Path('artifacts/analysis'))
    parser.add_argument('--count', type=int, default=2000000)
    parser.add_argument('--load-segment', type=lambda n:int(n,0), default=0x1000)
    args=parser.parse_args(); data=args.input.read_bytes(); args.output.mkdir(parents=True,exist_ok=True)
    BASE=args.load_segment
    if hashlib.sha256(data).hexdigest()!='a7ceeae290a7132880cd6ba616c2b0bb4001b2f7c83694f04052314fa26df6e7':
        raise ValueError('This recovered loader profile is bound to the preserved Panic release SHA-256')
    h,size,header=parse_mz(data)
    u=Uc(UC_ARCH_X86,UC_MODE_16); u.mem_map(0,2*1024*1024)
    u.mem_write(BASE*16,data[header:size]); u.mem_write((BASE-0x10)*16,b'\xcd\x20')
    for off in range(h[12],h[12]+h[3]*4,4):
        o,s=struct.unpack_from('<HH',data,off); a=BASE*16+s*16+o
        u.mem_write(a,struct.pack('<H',(struct.unpack('<H',u.mem_read(a,2))[0]+BASE)&65535))
    def get(n):return u.reg_read(REGS[n])
    def put(n,v):u.reg_write(REGS[n],v)
    def push(v):
        put('sp',(get('sp')-2)&65535); u.mem_write(get('ss')*16+get('sp'),struct.pack('<H',v&65535))
    def state():return {n:hex(get(n)) for n in REGS}
    for n,v in [('cs',BASE+h[11]),('ip',h[10]),('ss',BASE+h[7]),('sp',h[8]),('ds',BASE-16),('es',BASE-16),('eflags',0x202)]:put(n,v)
    trace=collections.deque(maxlen=120); interrupts=[]; stats=collections.Counter(); md=Cs(CS_ARCH_X86,CS_MODE_16)
    transitions=[]; relocations=[]; observed={}; state_meta={'last_cs':None,'decompressed_end':None,'handoff':None}
    def intr(uc,n,unused):
        stats[f'int{n:02x}']+=1
        if len(interrupts)<80:interrupts.append({'interrupt':n,**state()})
        if n==0x13:
            put('cx',0x3f); put('dx',0x0f80); put('eflags',get('eflags')&~1); return
        if n==0x10:
            put('ax',3); return
        if n in (0,1,2,3):
            flags=get('eflags'); push(flags);push(get('cs'));push(get('ip'))
            ip,cs=struct.unpack('<HH',u.mem_read(n*4,4))
            put('eflags',flags&~0x300);put('cs',cs);put('ip',ip); return
        raise RuntimeError(f'Unhandled interrupt {n:x}: {state()}')
    def code(uc,address,size,unused):
        stats['instructions']+=1
        raw=bytes(u.mem_read(address,size))
        key=(address,raw)
        observed[key]=observed.get(key,0)+1
        if get('cs')!=state_meta['last_cs']:
            transitions.append({'instruction_count':stats['instructions'],**state()})
            state_meta['last_cs']=get('cs')
            (args.output/f'loader-stage-{len(transitions):02d}-{get("cs"):04x}.bin').write_bytes(bytes(u.mem_read(BASE*16,0x14000)))
        # LZEXE 0.91 decoder's end marker and relocation instruction, verified
        # against the final relocated decoder byte sequence, not guessed data.
        if get('cs')==BASE+0x1223 and get('ip')==0xfc:
            state_meta['decompressed_end']=get('es')*16+get('di')
        if get('cs')==BASE+0x1223 and raw==b'\x26\x01\x1d':
            relocations.append(get('es')*16+get('di')-BASE*16)
        if get('cs')==BASE and get('ip')==0:
            state_meta['handoff']=state();u.emu_stop();return
        trace.append((hex(address),bytes(u.mem_read(address,size)).hex(),state()))
    def inp(uc,port,size,unused):return 0
    def outp(uc,port,size,value,unused):pass
    u.hook_add(UC_HOOK_INTR,intr);u.hook_add(UC_HOOK_CODE,code)
    u.hook_add(UC_HOOK_INSN,inp,None,1,0,UC_X86_INS_IN);u.hook_add(UC_HOOK_INSN,outp,None,1,0,UC_X86_INS_OUT)
    error=None
    try:u.emu_start(get('cs')*16+get('ip'),0,count=args.count)
    except Exception as exc:error=str(exc)
    result={'input_sha256':hashlib.sha256(data).hexdigest(),'header':h,'image_size':size,'initial_load_segment':hex(BASE),'final_state':state(),'stats':dict(stats),'error':error,'interrupts':interrupts,'transitions':transitions,'relocations':relocations,'recovery':state_meta,'last_instructions':[],
      'assumptions':['Unicorn x86 real mode, 2 MiB zero-initialized memory','PSP INT 20h bytes only; no complete DOS PSP model','INT 13h AH08 returns CX003f DX0f80, carry clear','INT 10h AH0f returns AX0003','All IN ports return zero; OUT operations have no host effects','Internal interrupt vectors 0..3 dispatched with real-mode stack semantics','Stops before original loader entry; DOS APIs and demo behavior not emulated']}
    for address,raw,regs in trace:
        ins=list(md.disasm(bytes.fromhex(raw),int(address,16)))
        result['last_instructions'].append({'address':address,'bytes':raw,'instruction':'; '.join(i.mnemonic+' '+i.op_str for i in ins),'registers':regs})
    (args.output/'loader-emulation.json').write_text(json.dumps(result,indent=2)+'\n')
    (args.output/'loader-emulated-memory.bin').write_bytes(bytes(u.mem_read(0,1024*1024)))
    lines=['; Instructions actually fetched during protection/decompression emulation.',
           '; Multiple byte versions at one address preserve self-modification.',
           '; Ordered by first observation; this is not original source or a static linear listing.']
    for (address,raw),count in observed.items():
        ins=list(md.disasm(raw,address))
        lines.append(f'{address:05x}  {raw.hex():30}  '+ '; '.join(i.mnemonic+' '+i.op_str for i in ins)+f' ; executions={count}')
    (args.output/'loader-observed-instructions.asm').write_text('\n'.join(lines)+'\n')
    if state_meta['handoff'] and state_meta['decompressed_end']:
        image=bytearray(u.mem_read(BASE*16,state_meta['decompressed_end']-BASE*16))
        for offset in relocations:
            value=struct.unpack_from('<H',image,offset)[0]
            struct.pack_into('<H',image,offset,(value-BASE)&65535)
        hdrlen=(0x1c+4*len(relocations)+15)&~15
        filelen=hdrlen+len(image); extra=max(0,(get('ss')-BASE)*16+get('sp')-len(image))
        headerout=bytearray(hdrlen)
        struct.pack_into('<14H',headerout,0,0x5a4d,filelen%512,(filelen+511)//512,len(relocations),hdrlen//16,(extra+15)//16,65535,get('ss')-BASE,get('sp'),0,0,0,0x1c,0)
        for i,offset in enumerate(relocations):struct.pack_into('<HH',headerout,0x1c+4*i,offset%16,offset//16)
        dest=args.output.parent/'derived'/'unpacked';dest.mkdir(parents=True,exist_ok=True)
        (dest/'PANIC.loader.exe').write_bytes(headerout+image)
        (dest/'PANIC.loader.module.bin').write_bytes(image)
        result['recovered_executable']={'path':str(dest/'PANIC.loader.exe'),'module_bytes':len(image),'relocations':len(relocations),'sha256':hashlib.sha256(headerout+image).hexdigest(),'note':'Synthetic MZ header; original initialized bytes recovered and load fixups reversed; overlay not appended.'}
        (args.output/'loader-emulation.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result[k] for k in ('final_state','stats','error')},indent=2))
    if not state_meta['handoff'] or not state_meta['decompressed_end'] or len(relocations)!=360:
        raise SystemExit('Recovery did not meet the expected handoff/relocation conditions')
if __name__=='__main__':main()

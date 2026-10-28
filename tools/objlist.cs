// Lists named kernel objects in this session's BaseNamedObjects (name<TAB>type).
using System;
using System.Runtime.InteropServices;

static class ObjList
{
    [StructLayout(LayoutKind.Sequential)] struct US { public ushort Len, Max; public IntPtr Buf; }
    [StructLayout(LayoutKind.Sequential)] struct OA { public int Length; public IntPtr Root; public IntPtr Name; public uint Attr; public IntPtr Sd, Sqos; }
    [StructLayout(LayoutKind.Sequential)] struct DirInfo { public US Name; public US Type; }
    [DllImport("ntdll")] static extern int NtOpenDirectoryObject(out IntPtr h, uint access, ref OA oa);
    [DllImport("ntdll")] static extern int NtQueryDirectoryObject(IntPtr h, IntPtr buf, int len, bool single, bool restart, ref uint ctx, out uint ret);

    static void Main()
    {
        int session = System.Diagnostics.Process.GetCurrentProcess().SessionId;
        string path = @"\Sessions\" + session + @"\BaseNamedObjects";
        var us = new US { Len = (ushort)(path.Length * 2), Max = (ushort)(path.Length * 2 + 2), Buf = Marshal.StringToHGlobalUni(path) };
        IntPtr pus = Marshal.AllocHGlobal(Marshal.SizeOf(us)); Marshal.StructureToPtr(us, pus, false);
        var oa = new OA { Length = Marshal.SizeOf(typeof(OA)), Name = pus, Attr = 0x40 };
        IntPtr h; int st = NtOpenDirectoryObject(out h, 1, ref oa);
        if (st != 0) { Console.Error.WriteLine("open " + st.ToString("X")); return; }
        uint ctx = 0, ret; IntPtr buf = Marshal.AllocHGlobal(1 << 16); bool restart = true;
        while (NtQueryDirectoryObject(h, buf, 1 << 16, true, restart, ref ctx, out ret) == 0)
        {
            restart = false;
            var d = (DirInfo)Marshal.PtrToStructure(buf, typeof(DirInfo));
            Console.WriteLine(Marshal.PtrToStringUni(d.Name.Buf, d.Name.Len / 2) + "\t" + Marshal.PtrToStringUni(d.Type.Buf, d.Type.Len / 2));
        }
    }
}

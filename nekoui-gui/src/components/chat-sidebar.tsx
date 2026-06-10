import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarMenuAction, SidebarTrigger
} from "@/components/ui/sidebar.tsx";
import {Button} from "@/components/ui/button.tsx";
import {SquarePen, Trash2, MoreVertical, Pencil, MessageSquare, Settings} from "lucide-react"
import {Separator} from "@/components/ui/separator.tsx";
import {useState, useEffect, useCallback} from "react";
import {useNavigate, useLocation} from "react-router-dom";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuPortal,
    DropdownMenuSub,
    DropdownMenuSubContent,
    DropdownMenuSubTrigger,
    DropdownMenuTrigger,
    DropdownMenuSeparator
} from "@/components/ui/dropdown-menu"
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog.tsx"
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip"
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import {Input} from "@/components/ui/input.tsx"

type SessionListItem = {
    session_id: string;
    title: string;
    created_at: string;
    last_active: string;
    message_turns: number;
};

export default function ChatSidebar() {
    const navigate = useNavigate();
    const location = useLocation();
    const sessionId = location.pathname.match(/\/sessions\/([^/]+)/)?.[1];
    const [sessionList, setSessionList] = useState<SessionListItem[]>([]);
    const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
    const [alertOpen, setAlertOpen] = useState(false);
    const [renameTarget, setRenameTarget] = useState<SessionListItem | null>(null);
    const [renameDialogOpen, setRenameDialogOpen] = useState(false);
    const [renameTitle, setRenameTitle] = useState("");

    const fetchSessionList = async () => {
        const res = await fetch(`/api/v1/sessions`, {method: "GET"})
        const data = await res.json();

        setSessionList(data.data)
    }

    const handleNewChatClick = useCallback(async () => {
        const res = await fetch(`/api/v1/sessions`, {method: "POST"});
        const data = await res.json();
        const newSessionId: string = data.data.session_id;
        await fetchSessionList();
        navigate(`/sessions/${newSessionId}`);
    }, [navigate]);

    const handleDeleteSessionClick = useCallback(async (id: string) => {
        await fetch(`/api/v1/sessions/${id}`, {method: "DELETE"});
        await fetchSessionList();
        navigate("/");
    }, [navigate])

    const handleRenameSessionClick = useCallback(async (id: string, title: string) => {
        await fetch(`/api/v1/sessions/${id}`, {method: "PATCH", body: JSON.stringify({title}), headers: {"Content-Type": "application/json"}});
        await fetchSessionList();
        setRenameDialogOpen(false);
        setRenameTarget(null);
    }, [navigate])

    useEffect(() => {
        fetchSessionList();
    }, []);

    return (
        <Sidebar collapsible="icon">
            <SidebarHeader>
                <Tooltip>
                    <TooltipTrigger asChild>
                        <SidebarTrigger size={"icon"} className="hover:bg-primary/10" />
                    </TooltipTrigger>
                    <TooltipContent side={"right"}>Toggle sidebar</TooltipContent>
                </Tooltip>
                <h1 className="scroll-m-20 p-2 text-xl tracking-wide group-data-[collapsible=icon]:hidden">NekoUI</h1>
                <SidebarMenu>
                    <SidebarMenuItem>
                        <SidebarMenuButton
                            onClick={handleNewChatClick}
                            tooltip={"Create a session"}
                            className="hover:bg-primary/10 cursor-pointer"
                        >
                            <SquarePen />
                            <span className="font-light group-data-[collapsible=icon]:hidden">New Chat</span>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarHeader>
             <Separator />
             <SidebarContent>
                 <SidebarGroup>
                     <SidebarMenu className="gap-0.5">
                         {sessionList.map((session) => (
                             <SidebarMenuItem key={session.session_id} className="group/session">
                                 <SidebarMenuButton
                                     asChild
                                     tooltip={session.title}
                                     className={`w-full pl-2 pr-9 cursor-pointer transition-all ${
                                         session.session_id === sessionId
                                             ? "bg-primary/10 hover:bg-primary/10 group-hover/session:bg-primary/10 font-medium"
                                             : "hover:bg-primary/10 group-hover/session:bg-primary/10"
                                     }`}
                                 >
                                     <a onClick={() => navigate(`/sessions/${session.session_id}`)}>
                                         <MessageSquare />
                                         <span className="font-light truncate group-data-[collapsible=icon]:hidden">{session.title}</span>
                                     </a>
                                 </SidebarMenuButton>
                                 <DropdownMenu>
                                     <DropdownMenuTrigger asChild>
                                         <SidebarMenuAction showOnHover>
                                             <MoreVertical />
                                             <span className="sr-only">More</span>
                                         </SidebarMenuAction>
                                     </DropdownMenuTrigger>
                                     <DropdownMenuContent className="w-48 rounded-lg">
                                         <DropdownMenuGroup>
                                             <DropdownMenuItem onSelect={(e) => { e.preventDefault(); setRenameTarget(session); setRenameTitle(session.title); setRenameDialogOpen(true); }}>
                                                 <Pencil className="text-muted-foreground" />
                                                 <span>Rename</span>
                                             </DropdownMenuItem>
                                         </DropdownMenuGroup>
                                         <DropdownMenuSeparator />
                                         <DropdownMenuGroup>
                                             <DropdownMenuItem variant="destructive" onSelect={(e) => { e.preventDefault(); setDeleteTarget(session.session_id); setAlertOpen(true); }}>
                                                 <Trash2 />
                                                 <span>Delete</span>
                                             </DropdownMenuItem>
                                         </DropdownMenuGroup>
                                     </DropdownMenuContent>
                                 </DropdownMenu>
                             </SidebarMenuItem>
                         ))}
                     </SidebarMenu>
                 </SidebarGroup>
             </SidebarContent>
             <Separator />
             <SidebarFooter className="p-1">
                 <DropdownMenu>
                     <DropdownMenuTrigger asChild>
                         <Button variant="ghost" className="hover:bg-primary/10">
                             <Settings />
                             <span className="group-data-[collapsible=icon]:hidden">Settings</span>
                         </Button>
                     </DropdownMenuTrigger>
                     <DropdownMenuContent className="w-70" align="start">
                         <DropdownMenuGroup>
                             <DropdownMenuItem>Settings</DropdownMenuItem>
                             <DropdownMenuSub>
                                 <DropdownMenuSubTrigger>Language</DropdownMenuSubTrigger>
                                 <DropdownMenuPortal>
                                     <DropdownMenuSubContent>
                                         <DropdownMenuItem>English</DropdownMenuItem>
                                         <DropdownMenuItem>日本語</DropdownMenuItem>
                                     </DropdownMenuSubContent>
                                 </DropdownMenuPortal>
                             </DropdownMenuSub>
                             <DropdownMenuItem>Help</DropdownMenuItem>
                         </DropdownMenuGroup>
                         <DropdownMenuSeparator />
                         <DropdownMenuGroup>
                             <DropdownMenuItem>logout</DropdownMenuItem>
                         </DropdownMenuGroup>
                     </DropdownMenuContent>
                 </DropdownMenu>
             </SidebarFooter>
             <AlertDialog open={alertOpen} onOpenChange={(open) => { setAlertOpen(open); if (!open) setDeleteTarget(null); }}>
                 <AlertDialogContent>
                     <AlertDialogHeader>
                         <AlertDialogTitle>Are you absolutely sure?</AlertDialogTitle>
                         <AlertDialogDescription>
                             This action cannot be undone. This will permanently delete this session.
                         </AlertDialogDescription>
                     </AlertDialogHeader>
                     <AlertDialogFooter>
                         <AlertDialogCancel>Cancel</AlertDialogCancel>
                         <AlertDialogAction variant={"destructive"} onClick={() => deleteTarget && handleDeleteSessionClick(deleteTarget)}>Delete</AlertDialogAction>
                     </AlertDialogFooter>
                 </AlertDialogContent>
             </AlertDialog>
             <Dialog open={renameDialogOpen} onOpenChange={(open) => { setRenameDialogOpen(open); if (!open) setRenameTarget(null); }}>
                 <DialogContent>
                     <DialogHeader>
                         <DialogTitle>Rename Session</DialogTitle>
                         <DialogDescription>Enter a new name for this session.</DialogDescription>
                     </DialogHeader>
                     <Input
                         value={renameTitle}
                         onChange={(e) => setRenameTitle(e.target.value)}
                         placeholder="Session name"
                         onKeyDown={(e) => { if (e.key === "Enter" && renameTarget) handleRenameSessionClick(renameTarget.session_id, renameTitle); }}
                     />
                     <DialogFooter>
                         <Button variant="outline" onClick={() => { setRenameDialogOpen(false); setRenameTarget(null); }}>Cancel</Button>
                         <Button onClick={() => renameTarget && handleRenameSessionClick(renameTarget.session_id, renameTitle)}>Save</Button>
                     </DialogFooter>
                 </DialogContent>
             </Dialog>
         </Sidebar>
     )
}
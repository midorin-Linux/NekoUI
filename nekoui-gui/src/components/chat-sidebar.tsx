import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem
} from "@/components/ui/sidebar.tsx";
import {Button} from "@/components/ui/button.tsx";
import {SquarePen} from "lucide-react"
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
} from "@/components/ui/dropdown-menu"

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

    useEffect(() => {
        fetchSessionList();
    }, []);

    return (
        <Sidebar>
            <SidebarHeader>
                <h1 className="scroll-m-20 p-2 text-xl tracking-wide">NekoUI</h1>
                <Button variant={"ghost"} onClick={handleNewChatClick} className="hover:bg-primary/10 cursor-pointer justify-start gap-3">
                    <SquarePen />
                    <p className="font-light">New Chat</p>
                </Button>
            </SidebarHeader>
            <Separator />
            <SidebarContent>
                <SidebarGroup>
                    <SidebarMenu>
                        <SidebarMenuItem>
                            <div className="flex flex-col gap-1">
                                {sessionList.map((session => (
                                    <SidebarMenuButton key={session.session_id} onClick={() => navigate(`/sessions/${session.session_id}`)} className={`hover:bg-primary/15 cursor-pointer transition-all pl-2 ${session.session_id === sessionId ? "bg-primary/10" : ""}`}>
                                        <p className="font-light">{session.title}</p>
                                    </SidebarMenuButton>
                                )))}
                            </div>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroup>
            </SidebarContent>
            <Separator />
            <SidebarFooter className="p-1">
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <Button variant="ghost" className="hover:bg-primary/10">Settings</Button>
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
                        <Separator className="my-1" />
                        <DropdownMenuGroup>
                            <DropdownMenuItem>logout</DropdownMenuItem>
                        </DropdownMenuGroup>
                    </DropdownMenuContent>
                </DropdownMenu>
            </SidebarFooter>
        </Sidebar>
    )
}
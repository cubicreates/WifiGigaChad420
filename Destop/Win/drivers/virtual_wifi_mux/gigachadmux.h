/*
 * WiFi GigaChad 420 — NDIS 6.80 Intermediate MUX Virtual Adapter Header (gigachadmux.h)
 *
 * ISOLATED EXPERIMENTAL KERNEL DRIVER (Phase 5 & Rule 5):
 * - Exposes upper-edge Virtual Miniport "WiFi GigaChad 420 Virtual Network" (NdisMedium802_3)
 * - Binds lower-edge NDIS Protocol to physical adapters
 * - Does NOT auto-install on user systems; requires WDK + Test-Signing VM.
 */

#ifndef _GIGACHAD_MUX_H_
#define _GIGACHAD_MUX_H_

#define NDIS680_MINIPORT 1
#define NDIS680 1
#include <ndis.h>

#define GIGACHAD_MUX_TAG 'xMcG'
#define GIGACHAD_MAX_PHYS_BINDINGS 8
#define GIGACHAD_LINK_SPEED_DEFAULT 1000000000ULL /* 1 Gbps nominal virtual link */

typedef struct _GIGA_PROTOCOL_BINDING {
    NDIS_HANDLE BindingHandle;
    UCHAR CurrentMacAddress[6];
    ULONG64 LinkSpeedBps;
    BOOLEAN IsConnected;
    LONG RefCount;
} GIGA_PROTOCOL_BINDING, *PGIGA_PROTOCOL_BINDING;

typedef struct _GIGA_VELAN_ADAPTER {
    NDIS_HANDLE MiniportAdapterHandle;
    UCHAR VirtualMacAddress[6];
    BOOLEAN Paused;
    LONG OutstandingSends;
    NDIS_SPIN_LOCK Lock;
} GIGA_VELAN_ADAPTER, *PGIGA_VELAN_ADAPTER;

DRIVER_INITIALIZE DriverEntry;
MINIPORT_INITIALIZE GigaMiniportInitializeEx;
MINIPORT_HALT GigaMiniportHaltEx;
MINIPORT_PAUSE GigaMiniportPause;
MINIPORT_RESTART GigaMiniportRestart;
MINIPORT_SEND_NET_BUFFER_LISTS GigaMiniportSendNetBufferLists;
MINIPORT_RETURN_NET_BUFFER_LISTS GigaMiniportReturnNetBufferLists;
MINIPORT_OID_REQUEST GigaMiniportOidRequest;
MINIPORT_CANCEL_OID_REQUEST GigaMiniportCancelOidRequest;

PROTOCOL_BIND_ADAPTER_EX GigaProtocolBindAdapterEx;
PROTOCOL_UNBIND_ADAPTER_EX GigaProtocolUnbindAdapterEx;

#endif /* _GIGACHAD_MUX_H_ */

